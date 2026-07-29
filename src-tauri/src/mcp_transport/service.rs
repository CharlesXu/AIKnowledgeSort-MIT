use super::auth::{credentials_from_headers, TransportCredentials};
use super::tools::{dispatch, requested_scope_id, tool_definition};
use crate::agent_access::authority::AgentAccessAuthority;
use crate::agent_access::schema::{
    AgentGrantSummary, AuthorizeRequest, Denial, DenialCode, IssuedSession, OpenSessionRequest,
};
use crate::profiles::ProfileAuthority;
use http::HeaderMap;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, InitializeRequestParams, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

struct AuthenticatedSession {
    grant: AgentGrantSummary,
    issued: IssuedSession,
    origin: Option<String>,
}

#[derive(Debug)]
pub enum CallFailure {
    Denied(Denial),
    Invalid(String),
    UnknownTool,
}

#[derive(Clone)]
pub struct GovernedMcpService {
    authority: AgentAccessAuthority,
    profiles: ProfileAuthority,
    config_root: PathBuf,
    session: Arc<Mutex<Option<AuthenticatedSession>>>,
}

impl GovernedMcpService {
    pub fn new(
        authority: AgentAccessAuthority,
        profiles: ProfileAuthority,
        config_root: PathBuf,
    ) -> Self {
        Self {
            authority,
            profiles,
            config_root,
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub fn initialize_from_headers(
        &self,
        headers: &HeaderMap,
        now: SystemTime,
    ) -> Result<AgentGrantSummary, Denial> {
        let mut session = self.session.lock().map_err(|_| {
            denial(
                DenialCode::AuthorityUnavailable,
                "MCP session state is unavailable",
            )
        })?;
        if session.is_some() {
            return Err(denial(
                DenialCode::SessionMismatch,
                "MCP session is already initialized",
            ));
        }
        let credentials = parse_credentials(headers)?;
        let grant = self.authority.verify_transport_credentials(
            &self.config_root,
            &credentials.grant_id,
            &credentials.agent_id,
            &credentials.grant_token,
            credentials.origin.as_deref(),
            now,
        )?;
        let issued = self.authority.open_session(
            &self.config_root,
            OpenSessionRequest {
                grant_id: credentials.grant_id,
                agent_id: credentials.agent_id,
                grant_token: credentials.grant_token,
                transport_origin: credentials.origin.clone(),
            },
            now,
        )?;
        *session = Some(AuthenticatedSession {
            grant: grant.clone(),
            issued,
            origin: credentials.origin,
        });
        Ok(grant)
    }

    pub fn list_tools_from_headers(
        &self,
        headers: &HeaderMap,
        mcp_request_id: &str,
        now: SystemTime,
    ) -> Result<Vec<Tool>, Denial> {
        let (credentials, session) = self.verify_request(headers, now)?;
        ensure_session_identity(&credentials, &session)?;
        let authorization_tool = session
            .grant
            .tool_ids
            .first()
            .ok_or_else(|| denial(DenialCode::ToolDenied, "Agent grant has no tools"))?
            .clone();
        self.authority.authorize_request(
            AuthorizeRequest {
                grant_id: credentials.grant_id,
                agent_id: credentials.agent_id,
                session_id: session.issued.session_id.clone(),
                session_token: session.issued.session_token,
                request_id: replay_identity(&session.issued.session_id, mcp_request_id),
                tool_id: authorization_tool,
                scope_id: None,
                request_bytes: 1,
                response_budget_bytes: session.grant.limits.max_response_bytes,
                transport_origin: credentials.origin,
            },
            now,
        )?;
        Ok(session
            .grant
            .tool_ids
            .iter()
            .filter_map(|tool_id| {
                crate::agent_access::schema::tool_catalog()
                    .iter()
                    .find(|tool| tool.tool_id == tool_id)
                    .copied()
            })
            .map(tool_definition)
            .collect())
    }

    pub fn call_tool_from_headers(
        &self,
        headers: &HeaderMap,
        mcp_request_id: &str,
        request: CallToolRequestParams,
        now: SystemTime,
    ) -> Result<Value, CallFailure> {
        let (credentials, session) = self
            .verify_request(headers, now)
            .map_err(CallFailure::Denied)?;
        ensure_session_identity(&credentials, &session).map_err(CallFailure::Denied)?;
        let descriptor = crate::agent_access::schema::tool_catalog()
            .iter()
            .find(|tool| tool.tool_id == request.name)
            .copied()
            .ok_or(CallFailure::UnknownTool)?;
        if !session
            .grant
            .tool_ids
            .iter()
            .any(|tool_id| tool_id == descriptor.tool_id)
        {
            return Err(CallFailure::Denied(denial(
                DenialCode::ToolDenied,
                "Agent tool is not granted",
            )));
        }
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|_| CallFailure::Invalid("Tool request cannot be serialized".to_owned()))?
            .len() as u64;
        let scope_id = requested_scope_id(descriptor.tool_id, &request.arguments)
            .map_err(CallFailure::Invalid)?;
        let replay_id = replay_identity(&session.issued.session_id, mcp_request_id);
        let authorized = self
            .authority
            .authorize_request(
                AuthorizeRequest {
                    grant_id: credentials.grant_id,
                    agent_id: credentials.agent_id,
                    session_id: session.issued.session_id,
                    session_token: session.issued.session_token,
                    request_id: replay_id,
                    tool_id: descriptor.tool_id.to_owned(),
                    scope_id,
                    request_bytes,
                    response_budget_bytes: session.grant.limits.max_response_bytes,
                    transport_origin: credentials.origin,
                },
                now,
            )
            .map_err(CallFailure::Denied)?;
        dispatch(
            &self.profiles,
            &session.grant,
            authorized,
            request.arguments,
        )
        .map_err(CallFailure::Invalid)
    }

    fn verify_request(
        &self,
        headers: &HeaderMap,
        now: SystemTime,
    ) -> Result<(TransportCredentials, AuthenticatedSession), Denial> {
        let credentials = parse_credentials(headers)?;
        let grant = self.authority.verify_transport_credentials(
            &self.config_root,
            &credentials.grant_id,
            &credentials.agent_id,
            &credentials.grant_token,
            credentials.origin.as_deref(),
            now,
        )?;
        let session = self
            .session
            .lock()
            .map_err(|_| {
                denial(
                    DenialCode::AuthorityUnavailable,
                    "MCP session state is unavailable",
                )
            })?
            .as_ref()
            .map(|session| AuthenticatedSession {
                grant,
                issued: IssuedSession {
                    session_id: session.issued.session_id.clone(),
                    session_token: session.issued.session_token.clone(),
                    expires_at_unix_ms: session.issued.expires_at_unix_ms,
                },
                origin: session.origin.clone(),
            })
            .ok_or_else(|| denial(DenialCode::UnknownSession, "MCP session is not initialized"))?;
        Ok((credentials, session))
    }
}

impl ServerHandler for GovernedMcpService {
    fn get_info(&self) -> ServerInfo {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Governed local read and review-only tools. No filesystem mutation is available.",
            )
            .with_server_info(rmcp::model::Implementation::new(
                "ai-knowledge-sort",
                env!("CARGO_PKG_VERSION"),
            ))
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        let headers = request_headers(&context)?;
        self.initialize_from_headers(headers, SystemTime::now())
            .map_err(denial_as_protocol_error)?;
        context.peer.set_peer_info(request.clone());
        let mut info = self.get_info();
        if ProtocolVersion::KNOWN_VERSIONS.contains(&request.protocol_version) {
            info.protocol_version = request.protocol_version;
        }
        Ok(info)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let headers = request_headers(&context)?;
        let request_id = serde_json::to_string(&context.id)
            .map_err(|_| McpError::internal_error("MCP request id is invalid", None))?;
        let tools = self
            .list_tools_from_headers(headers, &request_id, SystemTime::now())
            .map_err(denial_as_protocol_error)?;
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let headers = request_headers(&context)?;
        let request_id = serde_json::to_string(&context.id)
            .map_err(|_| McpError::internal_error("MCP request id is invalid", None))?;
        match self.call_tool_from_headers(headers, &request_id, request, SystemTime::now()) {
            Ok(result) => Ok(CallToolResult::structured(result)),
            Err(CallFailure::Denied(denied)) => Ok(CallToolResult::structured_error(json!({
                "status": "denied",
                "code": denied.code,
                "message": denied.message
            }))),
            Err(CallFailure::Invalid(message)) => Ok(CallToolResult::structured_error(json!({
                "status": "error",
                "code": "invalidToolRequest",
                "message": message
            }))),
            Err(CallFailure::UnknownTool) => {
                Err(McpError::invalid_params("Unknown governed tool", None))
            }
        }
    }
}

fn request_headers(context: &RequestContext<RoleServer>) -> Result<&HeaderMap, McpError> {
    context
        .extensions
        .get::<http::request::Parts>()
        .map(|parts| &parts.headers)
        .ok_or_else(|| {
            McpError::invalid_request("Authenticated HTTP request context is required", None)
        })
}

fn parse_credentials(headers: &HeaderMap) -> Result<TransportCredentials, Denial> {
    credentials_from_headers(headers)
        .map_err(|message| denial(DenialCode::Unauthenticated, message))
}

fn ensure_session_identity(
    credentials: &TransportCredentials,
    session: &AuthenticatedSession,
) -> Result<(), Denial> {
    if credentials.agent_id != session.grant.agent_id
        || credentials.grant_id != session.grant.grant_id
        || credentials.origin != session.origin
    {
        return Err(denial(
            DenialCode::SessionMismatch,
            "Transport credentials do not match the initialized MCP session",
        ));
    }
    Ok(())
}

fn replay_identity(session_id: &str, mcp_request_id: &str) -> String {
    let mut digest = Sha256::new();
    for value in [session_id.as_bytes(), mcp_request_id.as_bytes()] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    format!("{:x}", digest.finalize())
}

fn denial(code: DenialCode, message: impl Into<String>) -> Denial {
    Denial {
        code,
        message: message.into(),
    }
}

fn denial_as_protocol_error(denied: Denial) -> McpError {
    McpError::invalid_request(
        "Governed MCP authentication failed",
        Some(json!({"code": denied.code})),
    )
}

#[cfg(test)]
mod tests {
    use super::{CallFailure, GovernedMcpService};
    use crate::agent_access::authority::AgentAccessAuthority;
    use crate::agent_access::schema::{AgentResourceLimits, CreateAgentGrantRequest, DenialCode};
    use http::{header, HeaderMap, HeaderValue};
    use rmcp::model::CallToolRequestParams;
    use serde_json::{json, Map};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "aiks-mcp-{label}-{}-{}",
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

    fn setup() -> (
        TempTree,
        TempTree,
        AgentAccessAuthority,
        crate::agent_access::schema::IssuedAgentGrant,
        HeaderMap,
    ) {
        let config = TempTree::new("config");
        let scope = TempTree::new("scope");
        fs::write(scope.0.join("note.md"), "# Evidence\n\nVerified source.").unwrap();
        fs::write(scope.0.join("graph.json"), br#"{"nodes":["evidence"]}"#).unwrap();
        let authority = AgentAccessAuthority::default();
        let selection = authority
            .select_paths(vec![scope.0.clone()], now())
            .unwrap();
        let issued = authority
            .create_grant(
                &config.0,
                CreateAgentGrantRequest {
                    selection_id: selection.selection_id,
                    agent_id: "codex-desktop".to_owned(),
                    label: "Codex Desktop".to_owned(),
                    tool_ids: vec![
                        "capabilities.read".to_owned(),
                        "knowledge.read".to_owned(),
                        "graph.read".to_owned(),
                        "classification.propose".to_owned(),
                        "cleanup.suggest".to_owned(),
                    ],
                    allowed_http_origins: vec!["http://127.0.0.1:43123".to_owned()],
                    expires_in_seconds: 3_600,
                    limits: AgentResourceLimits {
                        max_requests_per_session: 100,
                        max_request_bytes: 128 * 1024,
                        max_response_bytes: 256 * 1024,
                    },
                },
                now(),
            )
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", issued.grant_token)).unwrap(),
        );
        headers.insert("x-aiks-agent-id", HeaderValue::from_static("codex-desktop"));
        headers.insert(
            "x-aiks-grant-id",
            HeaderValue::from_str(&issued.grant.grant_id).unwrap(),
        );
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:43123"),
        );
        (config, scope, authority, issued, headers)
    }

    fn args(value: serde_json::Value) -> Map<String, serde_json::Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn authenticates_once_and_lists_only_fixed_granted_tools() {
        let (config, _scope, authority, _issued, headers) = setup();
        let service = GovernedMcpService::new(
            authority,
            crate::profiles::ProfileAuthority::default(),
            config.0.clone(),
        );
        assert!(service
            .initialize_from_headers(&HeaderMap::new(), now())
            .is_err());
        service.initialize_from_headers(&headers, now()).unwrap();
        let tools = service
            .list_tools_from_headers(&headers, "list-1", now())
            .unwrap();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "capabilities.read",
                "knowledge.read",
                "graph.read",
                "classification.propose",
                "cleanup.suggest"
            ]
        );
        assert!(names
            .iter()
            .all(|name| !name.contains("delete") && !name.contains("execute")));
    }

    #[test]
    fn dispatches_bounded_reads_semantic_validation_and_rejects_replay() {
        let (config, _scope, authority, issued, headers) = setup();
        let service = GovernedMcpService::new(
            authority,
            crate::profiles::ProfileAuthority::default(),
            config.0.clone(),
        );
        service.initialize_from_headers(&headers, now()).unwrap();

        let knowledge = service
            .call_tool_from_headers(
                &headers,
                "request-1",
                CallToolRequestParams::new("knowledge.read").with_arguments(args(json!({
                    "scopeId": issued.grant.scopes[0].scope_id,
                    "relativePath": "note.md"
                }))),
                now(),
            )
            .unwrap();
        assert_eq!(knowledge["markdown"], "# Evidence\n\nVerified source.");

        let replay = service.call_tool_from_headers(
            &headers,
            "request-1",
            CallToolRequestParams::new("knowledge.read").with_arguments(args(json!({
                "scopeId": issued.grant.scopes[0].scope_id,
                "relativePath": "note.md"
            }))),
            now(),
        );
        assert!(matches!(
            replay,
            Err(CallFailure::Denied(ref denial)) if denial.code == DenialCode::ReplayedRequest
        ));

        let unavailable = service.call_tool_from_headers(
            &headers,
            "request-2",
            CallToolRequestParams::new("classification.propose").with_arguments(args(json!({
                "scopeId": issued.grant.scopes[0].scope_id,
                "sourceIdentity": {
                    "algorithm": "SHA-256",
                    "digest": "a".repeat(64)
                },
                "references": [{
                    "kind": "documentText",
                    "location": "page:1",
                    "text": "Evidence"
                }]
            }))),
            now(),
        );
        assert!(matches!(
            unavailable,
            Err(CallFailure::Invalid(ref message))
                if message == "Granted scope is not an initialized Vault"
        ));

        let cleanup = service
            .call_tool_from_headers(
                &headers,
                "request-3",
                CallToolRequestParams::new("cleanup.suggest").with_arguments(args(json!({
                    "facts": [
                        {
                            "name": "source.pdf",
                            "sha256": "a".repeat(64),
                            "sizeBytes": 42,
                            "sourceFormat": true
                        },
                        {
                            "name": "copy.pdf",
                            "sha256": "a".repeat(64),
                            "sizeBytes": 42,
                            "sourceFormat": false
                        }
                    ]
                }))),
                now(),
            )
            .unwrap();
        assert_eq!(cleanup["executionAvailable"], false);
        assert_eq!(cleanup["suggestions"][0]["retain"], "source.pdf");
        assert_eq!(cleanup["suggestions"][0]["reviewCandidates"][0], "copy.pdf");

        let ambiguous_cleanup = service.call_tool_from_headers(
            &headers,
            "request-4",
            CallToolRequestParams::new("cleanup.suggest").with_arguments(args(json!({
                "facts": [
                    {
                        "name": "duplicate.pdf",
                        "sha256": "b".repeat(64),
                        "sizeBytes": 42,
                        "sourceFormat": true
                    },
                    {
                        "name": "duplicate.pdf",
                        "sha256": "b".repeat(64),
                        "sizeBytes": 42,
                        "sourceFormat": false
                    }
                ]
            }))),
            now(),
        );
        assert!(matches!(
            ambiguous_cleanup,
            Err(CallFailure::Invalid(ref message))
                if message == "Cleanup fact names must be unique"
        ));
    }

    #[test]
    fn rechecks_headers_and_revocation_for_every_request() {
        let (config, _scope, authority, issued, headers) = setup();
        let service = GovernedMcpService::new(
            authority.clone(),
            crate::profiles::ProfileAuthority::default(),
            config.0.clone(),
        );
        service.initialize_from_headers(&headers, now()).unwrap();

        let mut spoofed = headers.clone();
        spoofed.insert("x-aiks-agent-id", HeaderValue::from_static("spoofed-agent"));
        assert!(service
            .list_tools_from_headers(&spoofed, "list-spoofed", now())
            .is_err());

        authority
            .revoke(&config.0, &issued.grant.grant_id, now())
            .unwrap();
        let denied = service
            .list_tools_from_headers(&headers, "list-revoked", now())
            .unwrap_err();
        assert_eq!(denied.code, DenialCode::RevokedGrant);
    }

    #[test]
    fn rejects_replayed_tool_listing_requests() {
        let (config, _scope, authority, _issued, headers) = setup();
        let service = GovernedMcpService::new(
            authority,
            crate::profiles::ProfileAuthority::default(),
            config.0.clone(),
        );
        service.initialize_from_headers(&headers, now()).unwrap();

        service
            .list_tools_from_headers(&headers, "list-replay", now())
            .unwrap();
        let denied = service
            .list_tools_from_headers(&headers, "list-replay", now())
            .unwrap_err();

        assert_eq!(denied.code, DenialCode::ReplayedRequest);
    }
}
