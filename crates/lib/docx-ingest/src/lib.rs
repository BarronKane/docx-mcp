//! HTTP ingest server for docx-mcp.
//!
//! Provides endpoints for submitting documentation payloads for ingestion.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Json, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use docx_core::control::{
    ControlError,
    CsharpIngestReport,
    CsharpIngestRequest,
    RustdocIngestReport,
    RustdocIngestRequest,
};
use docx_core::services::{RegistryError, SolutionRegistry};
use docx_core::store::StoreError;
use serde::{Deserialize, Serialize};
use surrealdb::Connection;
use tracing::info;

/// Configuration for the ingest HTTP server.
#[derive(Debug, Clone)]
pub struct IngestServerConfig {
    pub addr: SocketAddr,
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
}

impl IngestServerConfig {
    #[must_use]
    pub const fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            max_body_bytes: 25 * 1024 * 1024,
            request_timeout: Duration::from_secs(30),
        }
    }

    #[must_use]
    pub const fn with_max_body_bytes(mut self, max_body_bytes: usize) -> Self {
        self.max_body_bytes = max_body_bytes;
        self
    }

    #[must_use]
    pub const fn with_request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }
}

impl Default for IngestServerConfig {
    fn default() -> Self {
        Self::new("127.0.0.1:4010".parse().expect("valid default address"))
    }
}

/// HTTP ingest server wrapper.
pub struct IngestServer<C: Connection> {
    config: IngestServerConfig,
    state: AppState<C>,
}

impl<C: Connection> IngestServer<C> {
    #[must_use]
    pub const fn new(registry: Arc<SolutionRegistry<C>>, config: IngestServerConfig) -> Self {
        let state = AppState {
            registry,
            request_timeout: config.request_timeout,
        };
        Self { config, state }
    }
}

impl<C> IngestServer<C>
where
    C: Connection + Send + Sync + 'static,
{
    /// Runs the HTTP server until shutdown.
    ///
    /// # Errors
    /// Returns any listener or server error.
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self.config.addr;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let app = build_router(self.state, self.config.max_body_bytes);

        info!("docx-ingest listening on {addr}");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

struct AppState<C: Connection> {
    registry: Arc<SolutionRegistry<C>>,
    request_timeout: Duration,
}

impl<C: Connection> Clone for AppState<C> {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            request_timeout: self.request_timeout,
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn timeout() -> Self {
        Self {
            status: StatusCode::REQUEST_TIMEOUT,
            message: "ingest request timed out".to_string(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl From<RegistryError> for ApiError {
    fn from(err: RegistryError) -> Self {
        match err {
            RegistryError::UnknownSolution(solution) => {
                Self::not_found(format!("unknown solution: {solution}"))
            }
            RegistryError::CapacityReached { max } => Self::internal(format!(
                "solution registry capacity reached (max {max})"
            )),
            RegistryError::BuildFailed(message) => {
                Self::internal(format!("failed to build solution handle: {message}"))
            }
        }
    }
}

impl From<ControlError> for ApiError {
    fn from(err: ControlError) -> Self {
        match err {
            ControlError::Store(StoreError::InvalidInput(message)) => Self::bad_request(message),
            ControlError::Parse(parse_err) => Self::bad_request(parse_err.to_string()),
            ControlError::RustdocParse(parse_err) => Self::bad_request(parse_err.to_string()),
            ControlError::Store(StoreError::Surreal(err)) => Self::internal(err.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let payload = Json(ErrorResponse { error: self.message });
        (self.status, payload).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct CsharpIngestPayload {
    solution: String,
    #[serde(flatten)]
    request: CsharpIngestRequest,
}

#[derive(Debug, Deserialize)]
struct RustdocIngestPayload {
    solution: String,
    #[serde(flatten)]
    request: RustdocIngestRequest,
}

fn build_router<C>(state: AppState<C>, max_body_bytes: usize) -> Router
where
    C: Connection + Send + Sync + 'static,
{
    Router::new()
        .route("/health", get(health))
        .route("/ingest/csharp", post(ingest_csharp::<C>))
        .route("/ingest/rustdoc", post(ingest_rustdoc::<C>))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn ingest_csharp<C>(
    State(state): State<AppState<C>>,
    Json(payload): Json<CsharpIngestPayload>,
) -> Result<Json<CsharpIngestReport>, ApiError>
where
    C: Connection + Send + Sync + 'static,
{
    let control = control_for_solution(&state, &payload.solution).await?;
    let ingest = tokio::time::timeout(
        state.request_timeout,
        control.ingest_csharp_xml(payload.request),
    )
    .await
    .map_err(|_| ApiError::timeout())??;

    Ok(Json(ingest))
}

async fn ingest_rustdoc<C>(
    State(state): State<AppState<C>>,
    Json(payload): Json<RustdocIngestPayload>,
) -> Result<Json<RustdocIngestReport>, ApiError>
where
    C: Connection + Send + Sync + 'static,
{
    let control = control_for_solution(&state, &payload.solution).await?;
    let ingest = tokio::time::timeout(
        state.request_timeout,
        control.ingest_rustdoc_json(payload.request),
    )
    .await
    .map_err(|_| ApiError::timeout())??;

    Ok(Json(ingest))
}

async fn control_for_solution<C>(
    state: &AppState<C>,
    solution: &str,
) -> Result<docx_core::control::DocxControlPlane<C>, ApiError>
where
    C: Connection + Send + Sync + 'static,
{
    let trimmed = solution.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("solution is required"));
    }
    let handle = state.registry.get_or_init(trimmed).await.map_err(ApiError::from)?;
    Ok(handle.control())
}
