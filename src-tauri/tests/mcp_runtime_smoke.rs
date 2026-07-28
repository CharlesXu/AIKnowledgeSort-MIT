use ai_knowledge_sort_lib::agent_access::authority::AgentAccessAuthority;
use ai_knowledge_sort_lib::agent_access::schema::{
    AgentResourceLimits, CreateAgentGrantRequest, IssuedAgentGrant,
};
use ai_knowledge_sort_lib::mcp_transport::McpTransportAuthority;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "aiks-runtime-{label}-{}-{}",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_http_and_stdio_share_one_revocable_desktop_authority() {
    let config = TempTree::new("config");
    let scope = TempTree::new("scope");
    let authority = AgentAccessAuthority::default();
    let now = fixed_now();
    let selection = authority.select_paths(vec![scope.0.clone()], now).unwrap();
    let issued = authority
        .create_grant(
            &config.0,
            CreateAgentGrantRequest {
                selection_id: selection.selection_id,
                agent_id: "runtime-smoke".to_owned(),
                label: "Runtime smoke".to_owned(),
                tool_ids: vec!["capabilities.read".to_owned()],
                allowed_http_origins: Vec::new(),
                expires_in_seconds: 3_600,
                limits: AgentResourceLimits {
                    max_requests_per_session: 100,
                    max_request_bytes: 128 * 1024,
                    max_response_bytes: 256 * 1024,
                },
            },
            now,
        )
        .unwrap();
    let transport = McpTransportAuthority::default();
    let broker = transport
        .start(authority.clone(), config.0.clone(), 0)
        .await
        .unwrap();
    let url = broker.url.unwrap();

    direct_http_smoke(&url, &issued).await;

    let mut child = Command::new(env!("CARGO_BIN_EXE_ai-knowledge-sort"))
        .arg("--mcp-stdio-relay")
        .arg("--broker-url")
        .arg(&url)
        .env("AIKS_MCP_AGENT_ID", &issued.grant.agent_id)
        .env("AIKS_MCP_GRANT_ID", &issued.grant.grant_id)
        .env("AIKS_MCP_GRANT_TOKEN", &issued.grant_token)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = child.stderr.take().unwrap();

    write_message(&mut stdin, initialize(11)).await;
    let initialize_response = match next_line(&mut stdout).await {
        Some(response) => response,
        None => {
            let status = child.wait().await.unwrap();
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.unwrap();
            panic!(
                "stdio relay exited before initialize response: {status}; {}",
                String::from_utf8_lossy(&bytes)
            );
        }
    };
    assert_eq!(initialize_response["id"], 11);
    assert_eq!(
        initialize_response["result"]["serverInfo"]["name"],
        "ai-knowledge-sort"
    );

    write_message(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    )
    .await;
    write_message(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 12, "method": "tools/list", "params": {}}),
    )
    .await;
    let tools_response = next_line(&mut stdout).await.unwrap();
    assert_eq!(tools_response["id"], 12);
    assert_eq!(
        tools_response["result"]["tools"][0]["name"],
        "capabilities.read"
    );

    write_message(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "tools/call",
            "params": {"name": "capabilities.read", "arguments": {}}
        }),
    )
    .await;
    let capabilities_response = next_line(&mut stdout).await.unwrap();
    assert_eq!(capabilities_response["id"], 13);
    assert_eq!(
        capabilities_response["result"]["structuredContent"]["grantId"],
        issued.grant.grant_id
    );

    authority
        .revoke(&config.0, &issued.grant.grant_id, now)
        .unwrap();
    write_message(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 14, "method": "tools/list", "params": {}}),
    )
    .await;
    drop(stdin);
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("stdio relay exits after revocation")
        .unwrap();
    assert_eq!(status.code(), Some(2));
    let mut stderr_bytes = Vec::new();
    stderr.read_to_end(&mut stderr_bytes).await.unwrap();
    let stderr_text = String::from_utf8(stderr_bytes).unwrap();
    assert!(stderr_text.contains("HTTP status 403"));
    assert!(!stderr_text.contains(&issued.grant_token));

    assert!(!transport.stop().await.unwrap().running);
}

async fn direct_http_smoke(url: &str, issued: &IssuedAgentGrant) {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = authenticated(&client, url, issued, &initialize(1))
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
    assert!(response.text().await.unwrap().contains("ai-knowledge-sort"));

    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    });
    assert_eq!(
        authenticated(&client, url, issued, &initialized)
            .header("mcp-session-id", &session_id)
            .header("mcp-protocol-version", "2025-11-25")
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::ACCEPTED
    );

    let list = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let response = authenticated(&client, url, issued, &list)
        .header("mcp-session-id", &session_id)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(response.text().await.unwrap().contains("capabilities.read"));

    let call = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "capabilities.read", "arguments": {}}
    });
    let response = authenticated(&client, url, issued, &call)
        .header("mcp-session-id", &session_id)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains(&issued.grant.grant_id));
    assert!(!body.contains(&issued.grant_token));

    let replay = authenticated(&client, url, issued, &call)
        .header("mcp-session-id", &session_id)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), reqwest::StatusCode::OK);
    assert!(replay.text().await.unwrap().contains("replayedRequest"));

    let untrusted_origin = authenticated(&client, url, issued, &list)
        .header(ORIGIN, "http://127.0.0.1:43123")
        .header("mcp-session-id", &session_id)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .unwrap();
    assert_eq!(untrusted_origin.status(), reqwest::StatusCode::FORBIDDEN);

    let deleted = client
        .delete(url)
        .header(AUTHORIZATION, format!("Bearer {}", issued.grant_token))
        .header("x-aiks-agent-id", &issued.grant.agent_id)
        .header("x-aiks-grant-id", &issued.grant.grant_id)
        .header("mcp-session-id", session_id)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), reqwest::StatusCode::ACCEPTED);
}

fn authenticated(
    client: &reqwest::Client,
    url: &str,
    issued: &IssuedAgentGrant,
    body: &Value,
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

fn initialize(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "runtime-smoke", "version": "1.0.0"}
        }
    })
}

async fn write_message(stdin: &mut tokio::process::ChildStdin, message: Value) {
    stdin
        .write_all(message.to_string().as_bytes())
        .await
        .unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
}

async fn next_line(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
) -> Option<Value> {
    let line = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .expect("stdio relay response deadline")
        .unwrap()?;
    Some(serde_json::from_str(&line).unwrap())
}

fn fixed_now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(2_000_000_000)
}
