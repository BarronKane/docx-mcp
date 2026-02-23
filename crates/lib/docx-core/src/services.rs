use std::collections::{HashMap, HashSet};
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
use tokio::sync::RwLock;

use crate::control::DocxControlPlane;
use crate::store::SurrealDocStore;

/// Solution name reserved for internal namespace-discovery connections.
/// Ingestion into this name must be rejected to prevent polluting the DB.
pub const RESERVED_SOLUTION: &str = "__discovery__";

/// Future returned by the solution handle builder.
pub type BuildHandleFuture<C> =
    Pin<Box<dyn Future<Output = Result<Arc<SolutionHandle<C>>, RegistryError>> + Send + 'static>>;
/// Builder function that creates a solution handle for a solution name.
pub type BuildHandleFn<C> = Arc<dyn Fn(String) -> BuildHandleFuture<C> + Send + Sync + 'static>;

/// Future returned by the solution discovery function.
pub type DiscoverSolutionsFuture = Pin<Box<dyn Future<Output = Vec<String>> + Send + 'static>>;
/// Optional function that discovers existing solution names from the database
/// without requiring a specific database to be selected.
pub type DiscoverSolutionsFn = Arc<dyn Fn() -> DiscoverSolutionsFuture + Send + Sync + 'static>;

/// Configuration for the solution registry cache and builder.
#[derive(Clone)]
pub struct SolutionRegistryConfig<C: Connection> {
    /// Optional TTL for cached solutions.
    pub ttl: Option<Duration>,
    /// Sweep interval for the background eviction task.
    pub sweep_interval: Duration,
    /// Optional maximum number of cached solutions.
    pub max_entries: Option<usize>,
    /// Builder used to create solution handles.
    pub build_handle: BuildHandleFn<C>,
    /// Idle threshold before running a health check on next access.
    pub health_check_after: Duration,
    /// Optional function to discover existing solution names from the database
    /// at the namespace level (no specific database required).
    pub discover_solutions: Option<DiscoverSolutionsFn>,
}

impl<C: Connection> SolutionRegistryConfig<C> {
    #[must_use]
    pub fn new(build_handle: BuildHandleFn<C>) -> Self {
        Self {
            ttl: None,
            sweep_interval: Duration::from_secs(60),
            max_entries: None,
            build_handle,
            health_check_after: Duration::from_secs(60),
            discover_solutions: None,
        }
    }

    #[must_use]
    pub fn with_discover_solutions(mut self, f: DiscoverSolutionsFn) -> Self {
        self.discover_solutions = Some(f);
        self
    }

    #[must_use]
    pub const fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    #[must_use]
    pub const fn with_sweep_interval(mut self, sweep_interval: Duration) -> Self {
        self.sweep_interval = sweep_interval;
        self
    }

    #[must_use]
    pub const fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = Some(max_entries);
        self
    }

    #[must_use]
    pub const fn with_health_check_after(mut self, health_check_after: Duration) -> Self {
        self.health_check_after = health_check_after;
        self
    }
}

/// Errors produced by the solution registry.
#[derive(Debug)]
pub enum RegistryError {
    /// Unknown solution name was requested.
    UnknownSolution(String),
    /// Registry hit its configured capacity.
    CapacityReached { max: usize },
    /// Failed to build a solution handle.
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
    #[must_use]
    pub fn new(db: Arc<Surreal<C>>) -> Self {
        let store = SurrealDocStore::from_arc(db.clone());
        let control = DocxControlPlane::with_store(store.clone());
        Self { db, store, control }
    }

    #[must_use]
    pub fn from_surreal(db: Surreal<C>) -> Self {
        Self::new(Arc::new(db))
    }

    #[must_use]
    pub fn db(&self) -> Arc<Surreal<C>> {
        self.db.clone()
    }

    #[must_use]
    pub fn store(&self) -> SurrealDocStore<C> {
        self.store.clone()
    }

    #[must_use]
    pub fn control(&self) -> DocxControlPlane<C> {
        self.control.clone()
    }

    /// Lists all database names in the current namespace.
    pub async fn list_databases(&self) -> Vec<String> {
        self.store.list_databases().await.unwrap_or_default()
    }

    /// Runs a lightweight health check against the database connection.
    pub async fn ping(&self) -> bool {
        self.db
            .query("SELECT 1")
            .await
            .is_ok_and(|r| r.check().is_ok())
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
    handle: RwLock<Option<Arc<SolutionHandle<C>>>>,
    last_used_ms: AtomicU64,
}

impl<C: Connection> SolutionEntry<C> {
    fn new() -> Self {
        Self {
            handle: RwLock::new(None),
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
    #[must_use]
    pub fn new(config: SolutionRegistryConfig<C>) -> Self {
        Self {
            inner: Arc::new(SolutionRegistryInner {
                entries: RwLock::new(HashMap::new()),
                config,
            }),
        }
    }

    /// Gets the solution handle or builds it once if missing.
    ///
    /// If the handle has been idle longer than `health_check_after`, a ping is
    /// issued. On failure the stale handle is evicted and rebuilt.
    ///
    /// # Errors
    /// Returns `RegistryError` if capacity is exceeded or the build fails.
    pub async fn get_or_init(
        &self,
        solution: &str,
    ) -> Result<Arc<SolutionHandle<C>>, RegistryError> {
        let entry = {
            let map = self.inner.entries.read().await;
            map.get(solution).cloned()
        };

        let entry = if let Some(entry) = entry {
            entry
        } else {
            let mut map = self.inner.entries.write().await;
            if let Some(entry) = map.get(solution).cloned() {
                entry
            } else {
                if let Some(max_entries) = self.inner.config.max_entries
                    && map.len() >= max_entries
                {
                    return Err(RegistryError::CapacityReached { max: max_entries });
                }
                let entry = Arc::new(SolutionEntry::new());
                map.insert(solution.to_string(), entry.clone());
                entry
            }
        };

        // Try to get existing handle
        {
            let guard = entry.handle.read().await;
            if let Some(handle) = guard.as_ref() {
                // Health check if idle long enough
                let idle = entry.idle_for(now_ms());
                if idle <= self.inner.config.health_check_after || handle.ping().await {
                    entry.touch();
                    return Ok(handle.clone());
                }
                // Ping failed — fall through to rebuild
                tracing::debug!("health check failed for solution '{solution}', rebuilding");
            }
        }

        // Build or rebuild under write lock
        let mut guard = entry.handle.write().await;
        // Double-check: another task may have rebuilt while we waited
        if let Some(handle) = guard.as_ref()
            && handle.ping().await
        {
            entry.touch();
            return Ok(handle.clone());
        }
        let build_handle = self.inner.config.build_handle.clone();
        let handle = (build_handle)(solution.to_string()).await?;
        *guard = Some(handle.clone());
        drop(guard);
        entry.touch();
        Ok(handle)
    }

    /// Lists known solutions by merging the in-memory cache with a live DB
    /// discovery query (`INFO FOR NS`).
    ///
    /// When a `discover_solutions` function is configured it is called first;
    /// otherwise any live cached handle is used for the namespace query.  If
    /// neither is available the result falls back to the cache alone.
    pub async fn list_solutions(&self) -> Vec<String> {
        // Try the dedicated discovery function first (preferred path).
        let db_names: Vec<String> = if let Some(discover) = &self.inner.config.discover_solutions {
            (discover)().await
        } else {
            // Fallback: collect cached entries without holding the map lock, then
            // find any live handle to run INFO FOR NS through.
            let entries: Vec<Arc<SolutionEntry<C>>> = {
                let map = self.inner.entries.read().await;
                map.values().cloned().collect()
            };
            let mut live_handle: Option<Arc<SolutionHandle<C>>> = None;
            for entry in &entries {
                let guard = entry.handle.read().await;
                if let Some(h) = guard.as_ref() {
                    live_handle = Some(h.clone());
                    break;
                }
            }
            match live_handle {
                Some(h) => h.list_databases().await,
                None => vec![],
            }
        };

        // Merge DB-discovered names with cached names.
        let mut names: HashSet<String> = db_names.into_iter().collect();
        {
            let map = self.inner.entries.read().await;
            names.extend(map.keys().cloned());
        }
        let mut result: Vec<String> = names.into_iter().collect();
        result.sort();
        result
    }

    /// Removes a cached solution handle entry.
    pub async fn remove_solution(&self, solution: &str) -> bool {
        let mut map = self.inner.entries.write().await;
        map.remove(solution).is_some()
    }

    /// Evicts idle entries that exceed the configured TTL.
    pub async fn evict_idle(&self) -> usize {
        let Some(ttl) = self.inner.config.ttl else {
            return 0;
        };
        let now = now_ms();
        let mut map = self.inner.entries.write().await;
        let before = map.len();
        map.retain(|key, entry| {
            let keep = entry.idle_for(now) <= ttl;
            if !keep {
                tracing::debug!("evicted idle solution: {key}");
            }
            keep
        });
        before.saturating_sub(map.len())
    }

    #[must_use]
    /// Spawns a background task to evict idle entries on a schedule.
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
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use surrealdb::engine::local::{Db, Mem};

    fn build_test_registry(calls: Arc<AtomicUsize>, ttl: Option<Duration>) -> SolutionRegistry<Db> {
        let build: BuildHandleFn<Db> = Arc::new(move |solution: String| {
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
            config = config
                .with_ttl(ttl)
                .with_sweep_interval(Duration::from_millis(1));
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

    #[tokio::test]
    async fn registry_remove_solution_drops_cache_entry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = build_test_registry(calls.clone(), None);

        let _ = registry.get_or_init("alpha").await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        assert!(registry.remove_solution("alpha").await);
        let _ = registry.get_or_init("alpha").await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
