// === IMPORTS ===
use anyhow::Result;
use gitlab::api::{projects::Projects, AsyncQuery, Pagination};
use serde::Deserialize;

use super::RepositoryInfo;
use crate::service::create_gitlab_client;

// === ENUMS ===
/// Ошибки при работе с GitLab API
///
/// Представляет все возможные ошибки, которые могут возникнуть
/// при взаимодействии с GitLab API, включая сетевые проблемы,
/// ошибки аутентификации и проблемы с построением запросов.
#[allow(dead_code)] // Обработка ошибок для будущих расширений GitLab API
#[derive(thiserror::Error, Debug)]
pub enum GitlabError {
    /// Ошибка REST API запроса
    #[error(transparent)]
    Rest(#[from] gitlab::api::ApiError<gitlab::RestError>),
    
    /// Общая ошибка GitLab клиента
    #[error(transparent)]
    Gitlab(#[from] gitlab::GitlabError),
    
    /// Ошибка построения запроса к проектам
    #[error(transparent)]
    Projects(#[from] gitlab::api::projects::ProjectsBuilderError),
}

// === STRUCTS ===
/// Репозиторий GitLab из API ответа
///
/// Представляет основную информацию о репозитории GitLab,
/// полученную через REST API. Содержит минимальный набор полей,
/// необходимых для интеграции с системой управления репозиториями.
///
/// # Поля
///
/// * `id` - Уникальный идентификатор проекта в GitLab
/// * `path_with_namespace` - Полный путь проекта (namespace/project)
/// * `http_url_to_repo` - HTTP URL для клонирования репозитория
#[derive(Deserialize)]
pub struct GitlabRepository {
    /// Уникальный идентификатор проекта
    pub id: u128,
    /// Полный путь проекта включая namespace
    pub path_with_namespace: String,
    /// HTTP URL для клонирования
    pub http_url_to_repo: String,
}

// === FREE_FUNCTIONS ===
/// Получает все репозитории GitLab для пользователя
///
/// Выполняет аутентифицированный запрос к GitLab API для получения
/// всех репозиториев, к которым у пользователя есть доступ.
/// Использует пагинацию для получения полного списка.
///
/// # Arguments
///
/// * `access_token` - Токен доступа GitLab для аутентификации
/// * `api_base` - Базовый URL GitLab API (например, "https://gitlab.com")
///
/// # Returns
///
/// Возвращает вектор `RepositoryInfo` со стандартизированной информацией
/// о репозиториях для дальнейшей обработки в системе.
///
/// # Errors
///
/// Возвращает ошибку если:
/// - Токен доступа недействителен или истек
/// - GitLab API недоступен
/// - Нет прав доступа к репозиториям
/// - Ошибка сети при выполнении запроса
///
/// # Examples
///
/// ```rust
/// let repos = fetch_all_gitlab_repos(
///     "glpat-xxxxxxxxxxxxxxxxxxxx",
///     "https://gitlab.example.com"
/// ).await?;
///
/// for repo in repos {
///     println!("Repo: {} -> {}", repo.name, repo.git_url);
/// }
/// ```
pub async fn fetch_all_gitlab_repos(
    access_token: &str,
    api_base: &str,
) -> Result<Vec<RepositoryInfo>> {
    // Создаем аутентифицированный клиент GitLab
    let gitlab = create_gitlab_client(api_base, access_token).await?;
    
    // Получаем все проекты с членством пользователя через пагинацию
    // membership(true) ограничивает результаты только теми проектами,
    // где пользователь является участником
    let repos: Vec<GitlabRepository> = gitlab::api::paged(
        Projects::builder().membership(true).build()?,
        Pagination::All,
    )
    .query_async(&gitlab)
    .await?;

    // Преобразуем GitLab-специфичные структуры в универсальный формат
    Ok(repos
        .into_iter()
        .map(|repo| RepositoryInfo {
            name: repo.path_with_namespace,
            git_url: repo.http_url_to_repo,
            vendor_id: repo.id.to_string(),
        })
        .collect())
}
