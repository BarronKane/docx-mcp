use std::sync::Arc;

use docx_core::services::{
    BuildHandleFn, DiscoverSolutionsFn, RegistryError, SolutionHandle, SolutionRegistry,
    SolutionRegistryConfig,
};
use docx_core::store::SurrealDocStore;
use surrealdb::engine::any::{Any, connect};
use surrealdb::opt::auth::Namespace;

use crate::config::DocxConfig;

pub fn build_registry(config: &DocxConfig) -> SolutionRegistry<Any> {
    let config = config.clone();
    let build_config = config.clone();
    let build: BuildHandleFn<Any> = Arc::new(move |solution: String| {
        let config = build_config.clone();
        Box::pin(async move {
            let db = if config.db_in_memory {
                connect("mem://").await.map_err(map_build_error)?
            } else {
                let uri = config
                    .db_uri
                    .clone()
                    .ok_or_else(|| map_build_error("missing DOCX_DB_URI"))?;
                let username = config
                    .db_username
                    .clone()
                    .ok_or_else(|| map_build_error("missing DOCX_DB_USERNAME"))?;
                let password = config
                    .db_password
                    .clone()
                    .ok_or_else(|| map_build_error("missing DOCX_DB_PASSWORD"))?;
                let db = connect(uri).await.map_err(map_build_error)?;
                db.signin(Namespace {
                    namespace: config.db_namespace.clone(),
                    username,
                    password,
                })
                .await
                .map_err(map_build_error)?;
                db
            };

            let db_name = DocxConfig::db_name_for_solution(&solution);
            db.use_ns(&config.db_namespace)
                .use_db(db_name)
                .await
                .map_err(map_build_error)?;

            Ok(Arc::new(SolutionHandle::from_surreal(db)))
        })
    });

    // Discovery function: creates a namespace-scoped connection (no specific
    // database selected) so that INFO FOR NS returns all existing databases
    // without auto-creating a spurious one as a side-effect.
    let discover_config = config.clone();
    let discover: DiscoverSolutionsFn = Arc::new(move || {
        let config = discover_config.clone();
        Box::pin(async move {
            // In-memory mode: each solution is an isolated mem:// instance with
            // no shared namespace to enumerate.
            if config.db_in_memory {
                return vec![];
            }
            let (Some(uri), Some(username), Some(password)) = (
                config.db_uri.clone(),
                config.db_username.clone(),
                config.db_password.clone(),
            ) else {
                return vec![];
            };
            let Ok(db) = connect(uri).await else {
                return vec![];
            };
            if db
                .signin(Namespace {
                    namespace: config.db_namespace.clone(),
                    username,
                    password,
                })
                .await
                .is_err()
            {
                return vec![];
            }
            // Select only the namespace — no database — so INFO FOR NS works
            // without defining a new database as a side-effect.
            if db.use_ns(&config.db_namespace).await.is_err() {
                return vec![];
            }
            let store = SurrealDocStore::from_arc(Arc::new(db));
            store.list_databases().await.unwrap_or_default()
        })
    });

    let mut registry_config = SolutionRegistryConfig::new(build)
        .with_sweep_interval(config.sweep_interval)
        .with_health_check_after(config.health_check_after)
        .with_discover_solutions(discover);
    if let Some(ttl) = config.registry_ttl {
        registry_config = registry_config.with_ttl(ttl);
    }
    if let Some(max_entries) = config.max_entries {
        registry_config = registry_config.with_max_entries(max_entries);
    }

    SolutionRegistry::new(registry_config)
}

fn map_build_error(err: impl std::fmt::Display) -> RegistryError {
    RegistryError::BuildFailed(err.to_string())
}
