use std::sync::Arc;

use docx_core::services::{
    BuildHandleFn,
    RegistryError,
    SolutionHandle,
    SolutionRegistry,
    SolutionRegistryConfig,
};
use surrealdb::engine::any::{Any, connect};
use surrealdb::opt::auth::Root;

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
                db.signin(Root {
                    username: &username,
                    password: &password,
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

    let mut registry_config = SolutionRegistryConfig::new(build)
        .with_sweep_interval(config.sweep_interval);
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
