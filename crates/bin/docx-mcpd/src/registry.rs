use std::sync::Arc;

use docx_core::services::{
    BuildHandleFn,
    RegistryError,
    SolutionHandle,
    SolutionRegistry,
    SolutionRegistryConfig,
};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;

use crate::config::DocxConfig;

pub fn build_registry(config: &DocxConfig) -> SolutionRegistry<Client> {
    let config = config.clone();
    let build_config = config.clone();
    let build: BuildHandleFn<Client> = Arc::new(move |solution: String| {
        let config = build_config.clone();
        Box::pin(async move {
            let db = Surreal::new::<Ws>(&config.db_endpoint)
                .await
                .map_err(map_build_error)?;

            if let (Some(username), Some(password)) =
                (config.db_username.as_ref(), config.db_password.as_ref())
            {
                db.signin(Root {
                    username: username.as_str(),
                    password: password.as_str(),
                })
                .await
                .map_err(map_build_error)?;
            }

            let db_name = config.db_name_for_solution(&solution);
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
