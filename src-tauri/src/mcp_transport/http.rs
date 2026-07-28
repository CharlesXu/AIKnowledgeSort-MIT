use super::auth::credentials_from_headers;
use super::service::GovernedMcpService;
use crate::agent_access::authority::AgentAccessAuthority;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tower_http::limit::RequestBodyLimitLayer;

const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartMcpTransportRequest {
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTransportState {
    pub running: bool,
    pub url: Option<String>,
    pub executable_path: Option<String>,
}

struct BrokerRuntime {
    summary: McpTransportState,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

#[derive(Clone)]
struct HttpAuthState {
    authority: AgentAccessAuthority,
    config_root: PathBuf,
}

#[derive(Clone, Default)]
pub struct McpTransportAuthority {
    runtime: Arc<tokio::sync::Mutex<Option<BrokerRuntime>>>,
}

impl McpTransportAuthority {
    pub async fn start(
        &self,
        authority: AgentAccessAuthority,
        config_root: PathBuf,
        port: u16,
    ) -> Result<McpTransportState, String> {
        let mut runtime = self.runtime.lock().await;
        clear_finished(&mut runtime);
        if runtime.is_some() {
            return Err("Local MCP transport is already running".to_owned());
        }
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|error| format!("Local MCP loopback port cannot be bound: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("Local MCP loopback address is unavailable: {error}"))?;
        if !address.ip().is_loopback() {
            return Err("Local MCP listener did not bind to a loopback address".to_owned());
        }
        let url = format!("http://127.0.0.1:{}/mcp", address.port());
        let cancellation = CancellationToken::new();
        let factory_authority = authority.clone();
        let factory_config_root = config_root.clone();
        let mut server_config = StreamableHttpServerConfig::default()
            .with_cancellation_token(cancellation.child_token());
        server_config.stateful_mode = true;
        server_config.allowed_hosts = vec![format!("127.0.0.1:{}", address.port())];
        server_config.allowed_origins = Vec::new();
        server_config.session_store = None;
        let service: StreamableHttpService<GovernedMcpService, LocalSessionManager> =
            StreamableHttpService::new(
                move || {
                    Ok(GovernedMcpService::new(
                        factory_authority.clone(),
                        factory_config_root.clone(),
                    ))
                },
                Default::default(),
                server_config,
            );
        let auth_state = HttpAuthState {
            authority,
            config_root,
        };
        let router = Router::new()
            .route_service("/mcp", service)
            .layer(RequestBodyLimitLayer::new(MAX_HTTP_BODY_BYTES))
            .layer(middleware::from_fn_with_state(
                auth_state,
                enforce_http_boundary,
            ));
        let server_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(server_cancellation.cancelled_owned())
                .await;
        });
        let summary = McpTransportState {
            running: true,
            url: Some(url),
            executable_path: executable_path(),
        };
        *runtime = Some(BrokerRuntime {
            summary: summary.clone(),
            cancellation,
            task,
        });
        Ok(summary)
    }

    pub async fn inspect(&self) -> Result<McpTransportState, String> {
        let mut runtime = self.runtime.lock().await;
        clear_finished(&mut runtime);
        Ok(runtime
            .as_ref()
            .map(|current| current.summary.clone())
            .unwrap_or_else(stopped_state))
    }

    pub async fn stop(&self) -> Result<McpTransportState, String> {
        let current = self.runtime.lock().await.take();
        if let Some(runtime) = current {
            runtime.cancellation.cancel();
            tokio::time::timeout(std::time::Duration::from_secs(5), runtime.task)
                .await
                .map_err(|_| "Local MCP transport did not stop within 5 seconds".to_owned())?
                .map_err(|error| format!("Local MCP transport worker failed: {error}"))?;
        }
        Ok(stopped_state())
    }
}

async fn enforce_http_boundary(
    State(state): State<HttpAuthState>,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path() != "/mcp" {
        return StatusCode::NOT_FOUND.into_response();
    }
    if request.method() == Method::OPTIONS {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let credentials = match credentials_from_headers(request.headers()) {
        Ok(credentials) => credentials,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let grant = state.authority.verify_transport_credentials(
        &state.config_root,
        &credentials.grant_id,
        &credentials.agent_id,
        &credentials.grant_token,
        credentials.origin.as_deref(),
        SystemTime::now(),
    );
    let grant = match grant {
        Ok(grant) => grant,
        Err(denial) => {
            return match denial.code {
                crate::agent_access::schema::DenialCode::Unauthenticated
                | crate::agent_access::schema::DenialCode::UnknownGrant => {
                    StatusCode::UNAUTHORIZED.into_response()
                }
                _ => StatusCode::FORBIDDEN.into_response(),
            }
        }
    };
    if request
        .headers()
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > grant.limits.max_request_bytes)
    {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    next.run(request).await
}

fn clear_finished(runtime: &mut Option<BrokerRuntime>) {
    if runtime
        .as_ref()
        .is_some_and(|current| current.task.is_finished())
    {
        *runtime = None;
    }
}

fn stopped_state() -> McpTransportState {
    McpTransportState {
        running: false,
        url: None,
        executable_path: executable_path(),
    }
}

fn executable_path() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::McpTransportAuthority;
    use crate::agent_access::authority::AgentAccessAuthority;
    use crate::agent_access::schema::{AgentResourceLimits, CreateAgentGrantRequest};
    use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN};
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "aiks-http-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(2_000_000_000)
    }

    #[tokio::test]
    async fn hosts_only_authenticated_stateful_loopback_mcp() {
        let config = TempTree::new("config");
        let scope = TempTree::new("scope");
        let agent_access = AgentAccessAuthority::default();
        let selection = agent_access
            .select_paths(vec![scope.0.clone()], now())
            .unwrap();
        let issued = agent_access
            .create_grant(
                &config.0,
                CreateAgentGrantRequest {
                    selection_id: selection.selection_id,
                    agent_id: "test-agent".to_owned(),
                    label: "Test Agent".to_owned(),
                    tool_ids: vec!["capabilities.read".to_owned()],
                    allowed_http_origins: vec!["http://127.0.0.1:43123".to_owned()],
                    expires_in_seconds: 3_600,
                    limits: AgentResourceLimits {
                        max_requests_per_session: 10,
                        max_request_bytes: 128 * 1024,
                        max_response_bytes: 256 * 1024,
                    },
                },
                now(),
            )
            .unwrap();
        let transport = McpTransportAuthority::default();
        let state = transport
            .start(agent_access.clone(), config.0.clone(), 0)
            .await
            .unwrap();
        let url = state.url.unwrap();
        assert!(url.starts_with("http://127.0.0.1:"));
        let port = url::Url::parse(&url).unwrap().port().unwrap();
        let collision = McpTransportAuthority::default();
        assert!(collision
            .start(agent_access.clone(), config.0.clone(), port)
            .await
            .is_err());

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let init = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0.0"}
            }
        });
        let unauthorized = client
            .post(&url)
            .header(ACCEPT, "application/json, text/event-stream")
            .header(CONTENT_TYPE, "application/json")
            .json(&init)
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

        let untrusted_origin = authenticated(&client, &url, &issued, &init)
            .header(ORIGIN, "http://127.0.0.1:43124")
            .send()
            .await
            .unwrap();
        assert_eq!(untrusted_origin.status(), reqwest::StatusCode::FORBIDDEN);

        let response = authenticated(&client, &url, &issued, &init)
            .header(ORIGIN, "http://127.0.0.1:43123")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let session_id = response
            .headers()
            .get("mcp-session-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let body = response.text().await.unwrap();
        assert!(body.contains("ai-knowledge-sort"));

        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let initialized_response = authenticated(&client, &url, &issued, &initialized)
            .header(ORIGIN, "http://127.0.0.1:43123")
            .header("mcp-session-id", &session_id)
            .header("mcp-protocol-version", "2025-11-25")
            .send()
            .await
            .unwrap();
        assert_eq!(initialized_response.status(), reqwest::StatusCode::ACCEPTED);

        let list_tools = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        let list_response = authenticated(&client, &url, &issued, &list_tools)
            .header(ORIGIN, "http://127.0.0.1:43123")
            .header("mcp-session-id", &session_id)
            .header("mcp-protocol-version", "2025-11-25")
            .send()
            .await
            .unwrap();
        assert_eq!(list_response.status(), reqwest::StatusCode::OK);
        let list_body = list_response.text().await.unwrap();
        assert!(list_body.contains("capabilities.read"));
        assert!(!list_body.contains("delete"));

        let options = client
            .request(reqwest::Method::OPTIONS, &url)
            .send()
            .await
            .unwrap();
        assert_eq!(options.status(), reqwest::StatusCode::METHOD_NOT_ALLOWED);

        agent_access
            .revoke(&config.0, &issued.grant.grant_id, now())
            .unwrap();
        let revoked = authenticated(&client, &url, &issued, &list_tools)
            .header(ORIGIN, "http://127.0.0.1:43123")
            .header("mcp-session-id", &session_id)
            .header("mcp-protocol-version", "2025-11-25")
            .send()
            .await
            .unwrap();
        assert_eq!(revoked.status(), reqwest::StatusCode::FORBIDDEN);

        assert!(transport.inspect().await.unwrap().running);
        assert!(!transport.stop().await.unwrap().running);
        assert!(!transport.stop().await.unwrap().running);
    }

    fn authenticated(
        client: &reqwest::Client,
        url: &str,
        issued: &crate::agent_access::schema::IssuedAgentGrant,
        body: &serde_json::Value,
    ) -> reqwest::RequestBuilder {
        client
            .post(url)
            .header(ACCEPT, "application/json, text/event-stream")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {}", issued.grant_token))
            .header("x-aiks-agent-id", &issued.grant.agent_id)
            .header("x-aiks-grant-id", &issued.grant.grant_id)
            .json(body)
    }
}
