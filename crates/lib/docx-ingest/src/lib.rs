//! HTTP ingest server for docx-mcp.
//!
//! Provides endpoints for submitting documentation payloads for ingestion.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Json, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use docx_core::control::{
    ControlError, CsharpIngestReport, CsharpIngestRequest, RustdocIngestReport,
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
            RegistryError::CapacityReached { max } => {
                Self::internal(format!("solution registry capacity reached (max {max})"))
            }
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
        let payload = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, payload).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct CsharpIngestPayload {
    solution: Option<String>,
    project_id: Option<String>,
    xml: Option<String>,
    xml_path: Option<String>,
    ingest_id: Option<String>,
    source_path: Option<String>,
    source_modified_at: Option<String>,
    tool_version: Option<String>,
    source_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RustdocIngestPayload {
    solution: Option<String>,
    project_id: Option<String>,
    json: Option<String>,
    json_path: Option<String>,
    ingest_id: Option<String>,
    source_path: Option<String>,
    source_modified_at: Option<String>,
    tool_version: Option<String>,
    source_hash: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum IngestKind {
    CsharpXml,
    RustdocJson,
}

impl IngestKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CsharpXml => "csharp_xml",
            Self::RustdocJson => "rustdoc_json",
        }
    }
}
#[derive(Debug, Deserialize)]
struct IngestPayload {
    solution: Option<String>,
    project_id: Option<String>,
    kind: Option<IngestKind>,
    contents: Option<String>,
    contents_path: Option<String>,
    ingest_id: Option<String>,
    source_path: Option<String>,
    source_modified_at: Option<String>,
    tool_version: Option<String>,
    source_hash: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "report", rename_all = "snake_case")]
enum IngestResponse {
    CsharpXml(CsharpIngestReport),
    RustdocJson(RustdocIngestReport),
}

fn build_router<C>(state: AppState<C>, max_body_bytes: usize) -> Router
where
    C: Connection + Send + Sync + 'static,
{
    Router::new()
        .route("/health", get(health))
        .route("/ingest", post(ingest_payload::<C>))
        .route("/ingest/csharp", post(ingest_csharp::<C>))
        .route("/ingest/rustdoc", post(ingest_rustdoc::<C>))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

fn require_non_empty(field: &str, value: Option<String>) -> Result<String, ApiError> {
    value.map_or_else(
        || Err(ApiError::bad_request(format!("{field} is required"))),
        |value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Err(ApiError::bad_request(format!("{field} is required")))
            } else {
                Ok(trimmed.to_string())
            }
        },
    )
}

fn require_kind(kind: Option<IngestKind>) -> Result<IngestKind, ApiError> {
    kind.ok_or_else(|| ApiError::bad_request("kind is required (csharp_xml or rustdoc_json)"))
}

fn has_payload(value: Option<&String>) -> bool {
    value.is_some_and(|payload| !payload.trim().is_empty())
}

fn require_contents(
    contents: Option<&String>,
    contents_path: Option<&String>,
    kind: IngestKind,
) -> Result<(), ApiError> {
    if has_payload(contents) || has_payload(contents_path) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "contents or contents_path is required for {}",
            kind.as_str()
        )))
    }
}

async fn ingest_csharp<C>(
    State(state): State<AppState<C>>,
    Json(payload): Json<CsharpIngestPayload>,
) -> Result<Json<CsharpIngestReport>, ApiError>
where
    C: Connection + Send + Sync + 'static,
{
    let solution = require_non_empty("solution", payload.solution)?;
    let project_id = require_non_empty("project_id", payload.project_id)?;
    let control = control_for_solution(&state, &solution).await?;
    let request = CsharpIngestRequest {
        project_id,
        xml: payload.xml,
        xml_path: payload.xml_path,
        ingest_id: payload.ingest_id,
        source_path: payload.source_path,
        source_modified_at: payload.source_modified_at,
        tool_version: payload.tool_version,
        source_hash: payload.source_hash,
    };
    let ingest = tokio::time::timeout(state.request_timeout, control.ingest_csharp_xml(request))
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
    let solution = require_non_empty("solution", payload.solution)?;
    let project_id = require_non_empty("project_id", payload.project_id)?;
    let control = control_for_solution(&state, &solution).await?;
    let request = RustdocIngestRequest {
        project_id,
        json: payload.json,
        json_path: payload.json_path,
        ingest_id: payload.ingest_id,
        source_path: payload.source_path,
        source_modified_at: payload.source_modified_at,
        tool_version: payload.tool_version,
        source_hash: payload.source_hash,
    };
    let ingest = tokio::time::timeout(state.request_timeout, control.ingest_rustdoc_json(request))
        .await
        .map_err(|_| ApiError::timeout())??;

    Ok(Json(ingest))
}

async fn ingest_payload<C>(
    State(state): State<AppState<C>>,
    Json(payload): Json<IngestPayload>,
) -> Result<Json<IngestResponse>, ApiError>
where
    C: Connection + Send + Sync + 'static,
{
    let solution = require_non_empty("solution", payload.solution)?;
    let project_id = require_non_empty("project_id", payload.project_id)?;
    let kind = require_kind(payload.kind)?;
    require_contents(
        payload.contents.as_ref(),
        payload.contents_path.as_ref(),
        kind,
    )?;
    let control = control_for_solution(&state, &solution).await?;
    let ingest = match kind {
        IngestKind::CsharpXml => {
            let report = tokio::time::timeout(
                state.request_timeout,
                control.ingest_csharp_xml(CsharpIngestRequest {
                    project_id: project_id.clone(),
                    xml: payload.contents,
                    xml_path: payload.contents_path,
                    ingest_id: payload.ingest_id,
                    source_path: payload.source_path,
                    source_modified_at: payload.source_modified_at,
                    tool_version: payload.tool_version,
                    source_hash: payload.source_hash,
                }),
            )
            .await
            .map_err(|_| ApiError::timeout())??;
            IngestResponse::CsharpXml(report)
        }
        IngestKind::RustdocJson => {
            let report = tokio::time::timeout(
                state.request_timeout,
                control.ingest_rustdoc_json(RustdocIngestRequest {
                    project_id: project_id.clone(),
                    json: payload.contents,
                    json_path: payload.contents_path,
                    ingest_id: payload.ingest_id,
                    source_path: payload.source_path,
                    source_modified_at: payload.source_modified_at,
                    tool_version: payload.tool_version,
                    source_hash: payload.source_hash,
                }),
            )
            .await
            .map_err(|_| ApiError::timeout())??;
            IngestResponse::RustdocJson(report)
        }
    };

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
    let handle = state
        .registry
        .get_or_init(trimmed)
        .await
        .map_err(ApiError::from)?;
    Ok(handle.control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use docx_core::services::{BuildHandleFn, SolutionHandle, SolutionRegistryConfig};
    use serde_json::Value;
    use surrealdb::Surreal;
    use surrealdb::engine::local::{Db, Mem};
    use tower::ServiceExt;

    fn fixture_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("docx-core")
            .join("tests")
            .join("data")
            .join("docx_store_min.json")
    }

    fn load_fixture() -> String {
        let path = fixture_path();
        std::fs::read_to_string(&path).unwrap_or_else(|err| {
            let path_display = path.display();
            panic!("failed to read rustdoc fixture at {path_display}: {err}")
        })
    }

    fn build_registry() -> SolutionRegistry<Db> {
        let build: BuildHandleFn<Db> = Arc::new(move |solution: String| {
            Box::pin(async move {
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
        SolutionRegistry::new(SolutionRegistryConfig::new(build))
    }

    #[tokio::test]
    async fn ingest_payload_accepts_rustdoc_json() {
        let registry = Arc::new(build_registry());
        let state = AppState {
            registry,
            request_timeout: Duration::from_secs(5),
        };
        let app = build_router(state, 5 * 1024 * 1024);

        let body = serde_json::json!({
            "solution": "docx-mcp",
            "project_id": "docx-store",
            "kind": "rustdoc_json",
            "contents": load_fixture(),
            "ingest_id": "fixture",
            "source_path": "target/doc/docx_store_min.json",
            "tool_version": "fixture"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("failed to build request"),
            )
            .await
            .expect("ingest request failed");

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read response body");
        if status != StatusCode::OK {
            let body_text = String::from_utf8_lossy(&bytes);
            panic!("unexpected status {status}: {body_text}");
        }
        let payload: Value = serde_json::from_slice(&bytes).expect("response should be valid JSON");
        assert_eq!(
            payload.get("kind").and_then(Value::as_str),
            Some("rustdoc_json")
        );
        assert!(
            payload
                .get("report")
                .and_then(|value| value.get("symbol_count"))
                .is_some()
        );
    }

    #[tokio::test]
    async fn ingest_payload_accepts_contents_path() {
        let registry = Arc::new(build_registry());
        let state = AppState {
            registry,
            request_timeout: Duration::from_secs(5),
        };
        let app = build_router(state, 5 * 1024 * 1024);

        let temp_path = std::env::temp_dir().join("docx_ingest_fixture.json");
        std::fs::write(&temp_path, load_fixture()).expect("failed to write temp fixture");

        let body = serde_json::json!({
            "solution": "docx-mcp",
            "project_id": "docx-store",
            "kind": "rustdoc_json",
            "contents_path": temp_path.to_string_lossy(),
            "ingest_id": "fixture-path",
            "source_path": "target/doc/docx_store_min.json",
            "tool_version": "fixture"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("failed to build request"),
            )
            .await
            .expect("ingest request failed");

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read response body");
        if status != StatusCode::OK {
            let body_text = String::from_utf8_lossy(&bytes);
            panic!("unexpected status {status}: {body_text}");
        }
        let payload: Value = serde_json::from_slice(&bytes).expect("response should be valid JSON");
        assert_eq!(
            payload.get("kind").and_then(Value::as_str),
            Some("rustdoc_json")
        );
        let _ = std::fs::remove_file(&temp_path);
    }

    #[tokio::test]
    async fn ingest_payload_requires_solution() {
        let registry = Arc::new(build_registry());
        let state = AppState {
            registry,
            request_timeout: Duration::from_secs(5),
        };
        let app = build_router(state, 5 * 1024 * 1024);

        let body = serde_json::json!({
            "project_id": "docx-store",
            "kind": "rustdoc_json",
            "contents": load_fixture()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("failed to build request"),
            )
            .await
            .expect("ingest request failed");

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read response body");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let payload: Value = serde_json::from_slice(&bytes).expect("response should be valid JSON");
        assert_eq!(
            payload.get("error").and_then(Value::as_str),
            Some("solution is required")
        );
    }

    #[tokio::test]
    async fn ingest_payload_requires_contents() {
        let registry = Arc::new(build_registry());
        let state = AppState {
            registry,
            request_timeout: Duration::from_secs(5),
        };
        let app = build_router(state, 5 * 1024 * 1024);

        let body = serde_json::json!({
            "solution": "docx-mcp",
            "project_id": "docx-store",
            "kind": "rustdoc_json"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("failed to build request"),
            )
            .await
            .expect("ingest request failed");

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read response body");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let payload: Value = serde_json::from_slice(&bytes).expect("response should be valid JSON");
        assert_eq!(
            payload.get("error").and_then(Value::as_str),
            Some("contents or contents_path is required for rustdoc_json")
        );
    }

    #[tokio::test]
    async fn ingest_csharp_requires_project_id() {
        let registry = Arc::new(build_registry());
        let state = AppState {
            registry,
            request_timeout: Duration::from_secs(5),
        };
        let app = build_router(state, 5 * 1024 * 1024);

        let body = serde_json::json!({
            "solution": "docx-mcp"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ingest/csharp")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("failed to build request"),
            )
            .await
            .expect("ingest request failed");

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read response body");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let payload: Value = serde_json::from_slice(&bytes).expect("response should be valid JSON");
        assert_eq!(
            payload.get("error").and_then(Value::as_str),
            Some("project_id is required")
        );
    }
}
