use std::collections::HashSet;

use docx_store::models::Project;
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

use crate::store::StoreError;

use super::{ControlError, DocxControlPlane};

/// Input payload for upserting project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUpsertRequest {
    pub project_id: String,
    pub name: Option<String>,
    pub language: Option<String>,
    pub root_path: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl<C: Connection> DocxControlPlane<C> {
    /// Upserts a project and merges aliases.
    ///
    /// # Errors
    /// Returns `ControlError` if the input is invalid or the store operation fails.
    pub async fn upsert_project(
        &self,
        request: ProjectUpsertRequest,
    ) -> Result<Project, ControlError> {
        let ProjectUpsertRequest {
            project_id,
            name,
            language,
            root_path,
            description,
            aliases,
        } = request;

        if project_id.trim().is_empty() {
            return Err(ControlError::Store(StoreError::InvalidInput(
                "project_id is required".to_string(),
            )));
        }

        let mut project = self
            .store
            .get_project(&project_id)
            .await?
            .unwrap_or_else(|| Project {
                id: None,
                project_id: project_id.clone(),
                name: None,
                language: None,
                root_path: None,
                description: None,
                aliases: Vec::new(),
                search_text: None,
                extra: None,
            });

        if let Some(name) = name {
            project.name = Some(name);
        }
        if let Some(language) = language {
            project.language = Some(language);
        }
        if let Some(root_path) = root_path {
            project.root_path = Some(root_path);
        }
        if let Some(description) = description {
            project.description = Some(description);
        }

        merge_aliases(&mut project.aliases, &aliases);

        if project.name.is_none() && let Some(first_alias) = project.aliases.first() {
            project.name = Some(first_alias.clone());
        }

        project.search_text = build_project_search_text(&project);

        Ok(self.store.upsert_project(project).await?)
    }

    /// Fetches a project by id.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn get_project(&self, project_id: &str) -> Result<Option<Project>, ControlError> {
        Ok(self.store.get_project(project_id).await?)
    }

    /// Lists projects with an optional limit.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn list_projects(&self, limit: usize) -> Result<Vec<Project>, ControlError> {
        Ok(self.store.list_projects(limit).await?)
    }

    /// Searches projects by a name or alias pattern.
    ///
    /// # Errors
    /// Returns `ControlError` if the store query fails.
    pub async fn search_projects(
        &self,
        pattern: &str,
        limit: usize,
    ) -> Result<Vec<Project>, ControlError> {
        Ok(self.store.search_projects(pattern, limit).await?)
    }
}

fn merge_aliases(target: &mut Vec<String>, incoming: &[String]) {
    let mut seen: HashSet<String> = target
        .iter()
        .map(|alias| alias.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect();

    for alias in incoming {
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            target.push(trimmed.to_string());
        }
    }
}

fn build_project_search_text(project: &Project) -> Option<String> {
    let mut values = HashSet::new();
    let mut ordered = Vec::new();

    push_search_value(&mut values, &mut ordered, &project.project_id);
    if let Some(name) = project.name.as_ref() {
        push_search_value(&mut values, &mut ordered, name);
    }
    for alias in &project.aliases {
        push_search_value(&mut values, &mut ordered, alias);
    }

    if ordered.is_empty() {
        None
    } else {
        Some(ordered.join("|"))
    }
}

fn push_search_value(values: &mut HashSet<String>, ordered: &mut Vec<String>, input: &str) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return;
    }
    let lowered = trimmed.to_lowercase();
    if values.insert(lowered.clone()) {
        ordered.push(lowered);
    }
}
