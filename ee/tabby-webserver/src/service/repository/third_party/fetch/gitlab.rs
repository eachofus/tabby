// === IMPORTS ===
use anyhow::Result;
use serde::Deserialize;

use gitlab::api::{projects::Projects, AsyncQuery, Pagination};

use super::RepositoryInfo;
use crate::service::create_gitlab_client;

// === ENUMS ===
/// Error types that can occur during GitLab API operations
#[derive(thiserror::Error, Debug)]
#[allow(dead_code)] // Error enum defined for future error handling but not currently used
pub enum GitlabError {
    #[error(transparent)]
    Rest(#[from] gitlab::api::ApiError<gitlab::RestError>),
    #[error(transparent)]
    Gitlab(#[from] gitlab::GitlabError),
    #[error(transparent)]
    Projects(#[from] gitlab::api::projects::ProjectsBuilderError),
}

// === STRUCTS ===
/// GitLab repository representation from API response
#[derive(Deserialize)]
pub struct GitlabRepository {
    pub id: u128,
    pub path_with_namespace: String,
    pub http_url_to_repo: String,
}

pub async fn fetch_all_gitlab_repos(
    access_token: &str,
    api_base: &str,
) -> Result<Vec<RepositoryInfo>> {
    let gitlab = create_gitlab_client(api_base, access_token).await?;
    let repos: Vec<GitlabRepository> = gitlab::api::paged(
        Projects::builder().membership(true).build()?,
        Pagination::All,
    )
    .query_async(&gitlab)
    .await?;

    Ok(repos
        .into_iter()
        .map(|repo| RepositoryInfo {
            name: repo.path_with_namespace,
            git_url: repo.http_url_to_repo,
            vendor_id: repo.id.to_string(),
        })
        .collect())
}
