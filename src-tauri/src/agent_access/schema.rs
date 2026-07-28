use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const TOOL_CATALOG_VERSION: &str = "agent-tools-v1";
pub const MAX_ID_BYTES: usize = 128;
pub const MAX_LABEL_CHARS: usize = 256;
pub const MAX_SCOPES: usize = 16;
pub const MAX_TOOLS: usize = 16;
pub const MAX_HTTP_ORIGINS: usize = 8;
pub const MIN_GRANT_TTL_SECONDS: u64 = 60;
pub const MAX_GRANT_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MIN_REQUEST_BYTES: u64 = 1024;
pub const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
pub const MIN_RESPONSE_BYTES: u64 = 1024;
pub const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_REQUESTS_PER_SESSION: u32 = 100_000;
const TOKEN_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentToolEffect {
    Read,
    SemanticAdvice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolDescriptor {
    pub tool_id: &'static str,
    pub title: &'static str,
    pub effect: AgentToolEffect,
}

const TOOL_CATALOG: [AgentToolDescriptor; 6] = [
    AgentToolDescriptor {
        tool_id: "capabilities.read",
        title: "Read capability catalog",
        effect: AgentToolEffect::Read,
    },
    AgentToolDescriptor {
        tool_id: "knowledge.read",
        title: "Read authoritative knowledge",
        effect: AgentToolEffect::Read,
    },
    AgentToolDescriptor {
        tool_id: "graph.read",
        title: "Read evidence graph",
        effect: AgentToolEffect::Read,
    },
    AgentToolDescriptor {
        tool_id: "comparison.run",
        title: "Run semantic comparison",
        effect: AgentToolEffect::SemanticAdvice,
    },
    AgentToolDescriptor {
        tool_id: "classification.propose",
        title: "Propose classification",
        effect: AgentToolEffect::SemanticAdvice,
    },
    AgentToolDescriptor {
        tool_id: "cleanup.suggest",
        title: "Suggest duplicate cleanup",
        effect: AgentToolEffect::SemanticAdvice,
    },
];

pub fn tool_catalog() -> &'static [AgentToolDescriptor] {
    &TOOL_CATALOG
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResourceLimits {
    pub max_requests_per_session: u32,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAgentGrantRequest {
    pub selection_id: String,
    pub agent_id: String,
    pub label: String,
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub allowed_http_origins: Vec<String>,
    pub expires_in_seconds: u64,
    pub limits: AgentResourceLimits,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentScopeSummary {
    pub scope_id: String,
    pub display_path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentGrantStatus {
    Active,
    Inactive,
    Revoked,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentGrantSummary {
    pub grant_id: String,
    pub agent_id: String,
    pub label: String,
    pub tool_ids: Vec<String>,
    pub allowed_http_origins: Vec<String>,
    pub scopes: Vec<AgentScopeSummary>,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub revoked_at_unix_ms: Option<u64>,
    pub status: AgentGrantStatus,
    pub limits: AgentResourceLimits,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeScopeSelection {
    pub selection_id: String,
    pub scopes: Vec<AgentScopeSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedAgentGrant {
    pub grant: AgentGrantSummary,
    pub grant_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeAgentGrantRequest {
    pub grant_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccessState {
    pub schema_version: u32,
    pub tool_catalog_version: String,
    pub tools: Vec<AgentToolDescriptor>,
    pub grants: Vec<AgentGrantSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenSessionRequest {
    pub grant_id: String,
    pub agent_id: String,
    pub grant_token: String,
    #[serde(default)]
    pub transport_origin: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedSession {
    pub session_id: String,
    pub session_token: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorizeRequest {
    pub grant_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub session_token: String,
    pub request_id: String,
    pub tool_id: String,
    pub scope_id: Option<String>,
    pub request_bytes: u64,
    pub response_budget_bytes: u64,
    #[serde(default)]
    pub transport_origin: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DenialCode {
    InvalidRequest,
    Unauthenticated,
    UnknownGrant,
    InactiveGrant,
    RevokedGrant,
    ExpiredGrant,
    UnknownSession,
    SessionMismatch,
    OriginDenied,
    ReplayedRequest,
    ToolDenied,
    ScopeDenied,
    RequestLimitExceeded,
    RequestTooLarge,
    ResponseBudgetExceeded,
    AuthorityUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Denial {
    pub code: DenialCode,
    pub message: String,
}

impl CreateAgentGrantRequest {
    pub fn validate(&self) -> Result<(), String> {
        validate_safe_id("selection id", &self.selection_id)?;
        validate_safe_id("Agent id", &self.agent_id)?;
        validate_visible_label(&self.label)?;
        if self.tool_ids.is_empty() || self.tool_ids.len() > MAX_TOOLS {
            return Err("Agent grant must contain between 1 and 16 tools".to_owned());
        }
        let catalog_ids = tool_catalog()
            .iter()
            .map(|tool| tool.tool_id)
            .collect::<HashSet<_>>();
        let mut unique = HashSet::with_capacity(self.tool_ids.len());
        for tool_id in &self.tool_ids {
            if !catalog_ids.contains(tool_id.as_str()) {
                return Err(format!("Unsupported Agent tool: {tool_id}"));
            }
            if !unique.insert(tool_id.as_str()) {
                return Err(format!("Duplicate Agent tool: {tool_id}"));
            }
        }
        if self.allowed_http_origins.len() > MAX_HTTP_ORIGINS {
            return Err("Agent grant may allow at most 8 HTTP origins".to_owned());
        }
        let mut origins = HashSet::with_capacity(self.allowed_http_origins.len());
        for origin in &self.allowed_http_origins {
            let normalized = validate_http_origin(origin)?;
            if !origins.insert(normalized) {
                return Err(format!("Duplicate Agent HTTP origin: {origin}"));
            }
        }
        if !(MIN_GRANT_TTL_SECONDS..=MAX_GRANT_TTL_SECONDS).contains(&self.expires_in_seconds) {
            return Err("Agent grant expiry must be between 60 seconds and 30 days".to_owned());
        }
        self.limits.validate()
    }
}

pub fn validate_http_origin(value: &str) -> Result<String, String> {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("Agent HTTP origin contains unsafe characters".to_owned());
    }
    let parsed = url::Url::parse(value).map_err(|_| "Agent HTTP origin is invalid".to_owned())?;
    if parsed.scheme() != "http"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_none()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(
            "Agent HTTP origin must be a literal loopback HTTP origin with an explicit port"
                .to_owned(),
        );
    }
    let is_loopback = match parsed.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    };
    let normalized = parsed.origin().ascii_serialization();
    if !is_loopback || normalized != value {
        return Err(
            "Agent HTTP origin must be a canonical literal loopback HTTP origin".to_owned(),
        );
    }
    Ok(normalized)
}

impl AgentResourceLimits {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_REQUESTS_PER_SESSION).contains(&self.max_requests_per_session) {
            return Err("Agent session request limit is out of bounds".to_owned());
        }
        if !(MIN_REQUEST_BYTES..=MAX_REQUEST_BYTES).contains(&self.max_request_bytes) {
            return Err("Agent request byte limit is out of bounds".to_owned());
        }
        if !(MIN_RESPONSE_BYTES..=MAX_RESPONSE_BYTES).contains(&self.max_response_bytes) {
            return Err("Agent response byte limit is out of bounds".to_owned());
        }
        Ok(())
    }
}

pub fn validate_safe_id(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_ID_BYTES {
        return Err(format!("{field} must contain 1 through 128 bytes"));
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(format!("{field} is required"));
    };
    if !first.is_ascii_alphanumeric()
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!("{field} contains unsafe characters"));
    }
    Ok(())
}

fn validate_visible_label(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.chars().count() > MAX_LABEL_CHARS
        || value.chars().any(char::is_control)
    {
        return Err("Agent label must contain 1 through 256 visible characters".to_owned());
    }
    Ok(())
}

pub struct PlaintextToken(String);

impl PlaintextToken {
    pub fn into_string(self) -> String {
        self.0
    }
}

pub fn issue_token() -> Result<(PlaintextToken, String), String> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|_| "Secure Agent token generation is unavailable".to_owned())?;
    let plaintext = encode_hex(&bytes);
    let digest = token_digest(&plaintext);
    Ok((PlaintextToken(plaintext), digest))
}

pub fn verify_token(candidate: &str, expected_digest: &str) -> bool {
    let actual = token_digest(candidate);
    if actual.len() != expected_digest.len() {
        return false;
    }
    actual
        .bytes()
        .zip(expected_digest.bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn token_digest(token: &str) -> String {
    encode_hex(&Sha256::digest(token.as_bytes()))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        issue_token, tool_catalog, verify_token, AgentResourceLimits, AgentToolEffect,
        CreateAgentGrantRequest,
    };

    fn valid_request() -> CreateAgentGrantRequest {
        CreateAgentGrantRequest {
            selection_id: "selection-1".to_owned(),
            agent_id: "codex-desktop".to_owned(),
            label: "Codex Desktop".to_owned(),
            tool_ids: vec!["capabilities.read".to_owned(), "graph.read".to_owned()],
            allowed_http_origins: Vec::new(),
            expires_in_seconds: 3_600,
            limits: AgentResourceLimits {
                max_requests_per_session: 1_000,
                max_request_bytes: 128 * 1024,
                max_response_bytes: 256 * 1024,
            },
        }
    }

    #[test]
    fn catalog_contains_only_bounded_non_destructive_tools() {
        let catalog = tool_catalog();
        let ids = catalog.iter().map(|tool| tool.tool_id).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "capabilities.read",
                "knowledge.read",
                "graph.read",
                "comparison.run",
                "classification.propose",
                "cleanup.suggest",
            ]
        );
        assert_eq!(catalog[0].effect, AgentToolEffect::Read);
        assert_eq!(catalog[5].effect, AgentToolEffect::SemanticAdvice);
        assert!(ids.iter().all(|id| {
            !id.contains("delete")
                && !id.contains("execute")
                && !id.contains("move")
                && !id.contains("rename")
        }));
    }

    #[test]
    fn accepts_one_bounded_grant_request() {
        assert_eq!(valid_request().validate(), Ok(()));
    }

    #[test]
    fn rejects_unknown_fields_tools_ids_and_limits() {
        let unknown_field = serde_json::json!({
            "selectionId": "selection-1",
            "agentId": "codex-desktop",
            "label": "Codex Desktop",
            "toolIds": ["capabilities.read"],
            "allowedHttpOrigins": [],
            "expiresInSeconds": 3600,
            "limits": {
                "maxRequestsPerSession": 1000,
                "maxRequestBytes": 131072,
                "maxResponseBytes": 262144
            },
            "command": "rm"
        });
        assert!(serde_json::from_value::<CreateAgentGrantRequest>(unknown_field).is_err());

        let mut invalid = valid_request();
        invalid.agent_id = "../spoof".to_owned();
        assert!(invalid.validate().is_err());

        let mut invalid = valid_request();
        invalid.tool_ids = vec!["filesystem.delete".to_owned()];
        assert!(invalid.validate().is_err());

        let mut invalid = valid_request();
        invalid.tool_ids.push("graph.read".to_owned());
        assert!(invalid.validate().is_err());

        for ttl in [59, 30 * 24 * 60 * 60 + 1] {
            let mut invalid = valid_request();
            invalid.expires_in_seconds = ttl;
            assert!(invalid.validate().is_err());
        }

        for limits in [
            AgentResourceLimits {
                max_requests_per_session: 0,
                max_request_bytes: 128 * 1024,
                max_response_bytes: 256 * 1024,
            },
            AgentResourceLimits {
                max_requests_per_session: 100_001,
                max_request_bytes: 128 * 1024,
                max_response_bytes: 256 * 1024,
            },
            AgentResourceLimits {
                max_requests_per_session: 1_000,
                max_request_bytes: 1023,
                max_response_bytes: 256 * 1024,
            },
            AgentResourceLimits {
                max_requests_per_session: 1_000,
                max_request_bytes: 1024 * 1024 + 1,
                max_response_bytes: 256 * 1024,
            },
            AgentResourceLimits {
                max_requests_per_session: 1_000,
                max_request_bytes: 128 * 1024,
                max_response_bytes: 4 * 1024 * 1024 + 1,
            },
        ] {
            let mut invalid = valid_request();
            invalid.limits = limits;
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn accepts_only_bounded_literal_loopback_http_origins() {
        let mut request = valid_request();
        request.allowed_http_origins = vec![
            "http://127.0.0.1:43123".to_owned(),
            "http://[::1]:43123".to_owned(),
        ];
        assert_eq!(request.validate(), Ok(()));

        for origins in [
            vec!["http://127.0.0.1:43123", "http://127.0.0.1:43123"],
            vec!["http://0.0.0.0:43123"],
            vec!["http://localhost:43123"],
            vec!["http://user@127.0.0.1:43123"],
            vec!["http://127.0.0.1:43123/path"],
            vec!["http://127.0.0.1:43123?query=1"],
            vec!["http://127.0.0.1:43123#fragment"],
            vec!["http://127.0.0.1"],
            vec!["https://127.0.0.1:43123"],
            vec!["http://192.168.1.10:43123"],
            vec!["http://127.0.0.1:43123\n"],
        ] {
            let mut invalid = valid_request();
            invalid.allowed_http_origins = origins.into_iter().map(str::to_owned).collect();
            assert!(invalid.validate().is_err(), "accepted invalid origins");
        }

        let mut too_many = valid_request();
        too_many.allowed_http_origins = (0..9)
            .map(|offset| format!("http://127.0.0.1:{}", 43123 + offset))
            .collect();
        assert!(too_many.validate().is_err());
    }

    #[test]
    fn issues_high_entropy_tokens_and_keeps_only_verifiable_digests() {
        let (token, digest) = issue_token().expect("issue token");
        let plaintext = token.into_string();
        assert_eq!(plaintext.len(), 64);
        assert!(plaintext.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(digest.len(), 64);
        assert!(verify_token(&plaintext, &digest));
        assert!(!verify_token(&format!("x{}", &plaintext[1..]), &digest));
    }
}
