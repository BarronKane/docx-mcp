use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use surrealdb::{Connection, Surreal};
use tokio::sync::{OnceCell, RwLock};

use crate::control::DocxControlPlane;
use crate::store::SurrealDocStore;

pub type BuildHandleFuture<C> =
    Pin<Box<dyn Future<Output = Result<Arc<SolutionHandle<C>>, RegistryError>> + Send + 'static>>;
pub type BuildHandleFn<C> =
    Arc<dyn Fn(String) -> BuildHandleFuture<C> + Send + Sync + 'static>;

/// Configuration for the solution registry cache and builder.
#[derive(Clone)]
pub struct SolutionRegistryConfig<C: Connection> {
    pub ttl: Option<Duration>,
    pub sweep_interval: Duration,
    pub max_entries: Option<usize>,
    pub build_handle: BuildHandleFn<C>,
}

impl<C: Connection> SolutionRegistryConfig<C> {
    pub fn new(build_handle: BuildHandleFn<C>) -> Self {
        Self {
            ttl: None,
            sweep_interval: Duration::from_secs(60),
            max_entries: None,
            build_handle,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_sweep_interval(mut self, sweep_interval: Duration) -> Self {
        self.sweep_interval = sweep_interval;
        self
    }

    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = Some(max_entries);
        self
    }
}

#[derive(Debug)]
pub enum RegistryError {
    UnknownSolution(String),
    CapacityReached { max: usize },
    BuildFailed(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSolution(solution) => write!(f, "unknown solution: {solution}"),
            Self::CapacityReached { max } => {
                write!(f, "solution registry capacity reached (max {max})")
            }
            Self::BuildFailed(message) => write!(f, "failed to build solution handle: {message}"),
        }
    }
}

impl Error for RegistryError {}

/// Shared service handle for a single solution's database.
pub struct SolutionHandle<C: Connection> {
    db: Arc<Surreal<C>>,
    store: SurrealDocStore<C>,
    control: DocxControlPlane<C>,
}

impl<C: Connection> Clone for SolutionHandle<C> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            store: self.store.clone(),
            control: self.control.clone(),
        }
    }
}

impl<C: Connection> SolutionHandle<C> {
    pub fn new(db: Arc<Surreal<C>>) -> Self {
        let store = SurrealDocStore::from_arc(db.clone());
        let control = DocxControlPlane::with_store(store.clone());
        Self { db, store, control }
    }

    pub fn from_surreal(db: Surreal<C>) -> Self {
        Self::new(Arc::new(db))
    }

    pub fn db(&self) -> Arc<Surreal<C>> {
        self.db.clone()
    }

    pub fn store(&self) -> SurrealDocStore<C> {
        self.store.clone()
    }

    pub fn control(&self) -> DocxControlPlane<C> {
        self.control.clone()
    }
}

/// Registry for dynamically created solution handles.
#[derive(Clone)]
pub struct SolutionRegistry<C: Connection> {
    inner: Arc<SolutionRegistryInner<C>>,
}

/// Internal registry state shared across clones.
struct SolutionRegistryInner<C: Connection> {
    entries: RwLock<HashMap<String, Arc<SolutionEntry<C>>>>,
    config: SolutionRegistryConfig<C>,
}

/// Cache entry that tracks a solution handle and last access time.
struct SolutionEntry<C: Connection> {
    handle: OnceCell<Arc<SolutionHandle<C>>>,
    last_used_ms: AtomicU64,
}

impl<C: Connection> SolutionEntry<C> {
    fn new() -> Self {
        Self {
            handle: OnceCell::new(),
            last_used_ms: AtomicU64::new(now_ms()),
        }
    }

    fn touch(&self) {
        self.last_used_ms.store(now_ms(), Ordering::Relaxed);
    }

    fn idle_for(&self, now_ms: u64) -> Duration {
        let last = self.last_used_ms.load(Ordering::Relaxed);
        Duration::from_millis(now_ms.saturating_sub(last))
    }
}

impl<C: Connection> SolutionRegistry<C> {
    pub fn new(config: SolutionRegistryConfig<C>) -> Self {
        Self {
            inner: Arc::new(SolutionRegistryInner {
                entries: RwLock::new(HashMap::new()),
                config,
            }),
        }
    }

    pub async fn get_or_init(
        &self,
        solution: &str,
    ) -> Result<Arc<SolutionHandle<C>>, RegistryError> {
        let entry = {
            let map = self.inner.entries.read().await;
            map.get(solution).cloned()
        };

        let entry = match entry {
            Some(entry) => entry,
            None => {
                let mut map = self.inner.entries.write().await;
                if let Some(entry) = map.get(solution).cloned() {
                    entry
                } else {
                    if let Some(max_entries) = self.inner.config.max_entries {
                        if map.len() >= max_entries {
                            return Err(RegistryError::CapacityReached { max: max_entries });
                        }
                    }
                    let entry = Arc::new(SolutionEntry::new());
                    map.insert(solution.to_string(), entry.clone());
                    entry
                }
            }
        };

        entry.touch();

        let build_handle = self.inner.config.build_handle.clone();
        let handle = entry
            .handle
            .get_or_try_init(|| (build_handle)(solution.to_string()))
            .await?;
        Ok(handle.clone())
    }

    pub async fn list_solutions(&self) -> Vec<String> {
        let map = self.inner.entries.read().await;
        map.keys().cloned().collect()
    }

    pub async fn evict_idle(&self) -> usize {
        let Some(ttl) = self.inner.config.ttl else {
            return 0;
        };
        let now = now_ms();
        let mut map = self.inner.entries.write().await;
        let before = map.len();
        map.retain(|_, entry| entry.idle_for(now) <= ttl);
        before.saturating_sub(map.len())
    }

    pub fn spawn_sweeper(self) -> Option<tokio::task::JoinHandle<()>>
    where
        C: Send + Sync + 'static,
    {
        let _ttl = self.inner.config.ttl?;
        let interval = self.inner.config.sweep_interval;
        let registry = self;
        Some(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let _ = registry.evict_idle().await;
            }
        }))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use surrealdb::engine::local::Mem;

    fn build_test_registry(
        calls: Arc<AtomicUsize>,
        ttl: Option<Duration>,
    ) -> SolutionRegistry<Mem> {
        let build = Arc::new(move |solution: String| {
            let calls = calls.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                let db = Surreal::new::<Mem>(())
                    .await
                    .map_err(|err| RegistryError::BuildFailed(err.to_string()))?;
                db.use_ns("docx")
                    .use_db(&solution)
                    .await
                    .map_err(|err| RegistryError::BuildFailed(err.to_string()))?;
                Ok(Arc::new(SolutionHandle::from_surreal(db)))
            })
        });

        let mut config = SolutionRegistryConfig::new(build);
        if let Some(ttl) = ttl {
            config = config.with_ttl(ttl).with_sweep_interval(Duration::from_millis(1));
        }
        SolutionRegistry::new(config)
    }

    #[tokio::test]
    async fn registry_single_flight() {
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = build_test_registry(calls.clone(), None);

        let r1 = registry.clone();
        let r2 = registry.clone();
        let (left, right) = tokio::join!(r1.get_or_init("alpha"), r2.get_or_init("alpha"));
        assert!(left.is_ok());
        assert!(right.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn registry_evicts_idle_entries() {
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = build_test_registry(calls, Some(Duration::from_millis(1)));

        let _ = registry.get_or_init("alpha").await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        let evicted = registry.evict_idle().await;
        assert_eq!(evicted, 1);
    }
}
