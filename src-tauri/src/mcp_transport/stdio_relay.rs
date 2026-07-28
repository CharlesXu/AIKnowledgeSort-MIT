use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::time::Duration;

const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_ID_ENV: &str = "AIKS_MCP_AGENT_ID";
const GRANT_ID_ENV: &str = "AIKS_MCP_GRANT_ID";
const GRANT_TOKEN_ENV: &str = "AIKS_MCP_GRANT_TOKEN";

struct RelayConfig {
    broker_url: String,
    agent_id: String,
    grant_id: String,
    grant_token: String,
}

struct RelayState {
    session_id: Option<String>,
    protocol_version: Option<String>,
}

pub fn maybe_run_from_process_args() -> Result<bool, String> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).and_then(|value| value.to_str()) != Some("--mcp-stdio-relay") {
        return Ok(false);
    }
    let config = parse_config(&args, |name| std::env::var(name).ok())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| "MCP stdio relay runtime cannot be started".to_owned())?;
    runtime.block_on(run_relay(
        config,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    ))?;
    Ok(true)
}

async fn run_relay(
    config: RelayConfig,
    input: impl Read,
    output: impl Write,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| "MCP stdio relay HTTP client cannot be created".to_owned())?;
    let headers = credential_headers(&config)?;
    let mut state = RelayState {
        session_id: None,
        protocol_version: None,
    };
    let mut reader = BufReader::new(input);
    let mut writer = BufWriter::new(output);
    while let Some(line) = read_frame(&mut reader)? {
        let message: Value = serde_json::from_slice(&line)
            .map_err(|_| "MCP stdio input is not one valid JSON object".to_owned())?;
        let object = message
            .as_object()
            .ok_or_else(|| "MCP stdio input must be one JSON object, not a batch".to_owned())?;
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err("MCP stdio input must use JSON-RPC 2.0".to_owned());
        }
        let is_notification = object.get("id").is_none();
        if object.get("method").and_then(Value::as_str) == Some("initialize") {
            if state.session_id.is_some() {
                return Err("MCP stdio relay is already initialized".to_owned());
            }
            state.protocol_version = object
                .get("params")
                .and_then(|params| params.get("protocolVersion"))
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        let mut request = client
            .post(&config.broker_url)
            .headers(headers.clone())
            .body(line);
        if let Some(session_id) = &state.session_id {
            request = request.header("mcp-session-id", session_id);
        }
        if let Some(protocol_version) = &state.protocol_version {
            request = request.header("mcp-protocol-version", protocol_version);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "MCP broker request failed".to_owned())?;
        if let Some(session_id) = response.headers().get("mcp-session-id") {
            let session_id = session_id
                .to_str()
                .map_err(|_| "MCP broker returned an invalid session id".to_owned())?;
            if state
                .session_id
                .as_deref()
                .is_some_and(|current| current != session_id)
            {
                return Err("MCP broker changed the authenticated session id".to_owned());
            }
            state.session_id = Some(session_id.to_owned());
        }
        if response.status() == reqwest::StatusCode::ACCEPTED && is_notification {
            continue;
        }
        if !response.status().is_success() {
            return Err(format!(
                "MCP broker rejected the request with HTTP status {}",
                response.status().as_u16()
            ));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let body = bounded_response(response).await?;
        let protocol_line = if content_type.starts_with("application/json") {
            validate_protocol_response(&body)?
        } else if content_type.starts_with("text/event-stream") {
            extract_sse_response(&body)?
        } else {
            return Err("MCP broker returned an unsupported content type".to_owned());
        };
        writer
            .write_all(&protocol_line)
            .and_then(|_| writer.write_all(b"\n"))
            .and_then(|_| writer.flush())
            .map_err(|_| "MCP stdio response cannot be written".to_owned())?;
    }
    Ok(())
}

fn parse_config(
    args: &[OsString],
    env: impl Fn(&str) -> Option<String>,
) -> Result<RelayConfig, String> {
    if args.len() != 4
        || args.get(1).and_then(|value| value.to_str()) != Some("--mcp-stdio-relay")
        || args.get(2).and_then(|value| value.to_str()) != Some("--broker-url")
    {
        return Err(
            "MCP stdio relay requires exactly --mcp-stdio-relay --broker-url <loopback-url>"
                .to_owned(),
        );
    }
    let broker_url = args[3]
        .to_str()
        .ok_or_else(|| "MCP broker URL must be UTF-8".to_owned())?;
    validate_broker_url(broker_url)?;
    let agent_id = required_env(&env, AGENT_ID_ENV)?;
    let grant_id = required_env(&env, GRANT_ID_ENV)?;
    let grant_token = required_env(&env, GRANT_TOKEN_ENV)?;
    if !valid_id(&agent_id) || !valid_id(&grant_id) || !valid_token(&grant_token) {
        return Err("MCP stdio relay credentials are invalid".to_owned());
    }
    Ok(RelayConfig {
        broker_url: broker_url.to_owned(),
        agent_id,
        grant_id,
        grant_token,
    })
}

fn validate_broker_url(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("MCP broker URL contains unsafe characters".to_owned());
    }
    let url = url::Url::parse(value).map_err(|_| "MCP broker URL is invalid".to_owned())?;
    let loopback = match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    };
    if url.scheme() != "http"
        || !loopback
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/mcp"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.as_str() != value
    {
        return Err(
            "MCP broker URL must be one canonical literal-loopback HTTP /mcp URL".to_owned(),
        );
    }
    Ok(())
}

fn credential_headers(config: &RelayConfig) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.grant_token))
            .map_err(|_| "MCP bearer credential is invalid".to_owned())?,
    );
    headers.insert(
        "x-aiks-agent-id",
        HeaderValue::from_str(&config.agent_id)
            .map_err(|_| "MCP Agent id is invalid".to_owned())?,
    );
    headers.insert(
        "x-aiks-grant-id",
        HeaderValue::from_str(&config.grant_id)
            .map_err(|_| "MCP grant id is invalid".to_owned())?,
    );
    Ok(headers)
}

fn read_frame(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut frame = Vec::new();
    let bytes = reader
        .take((MAX_FRAME_BYTES + 2) as u64)
        .read_until(b'\n', &mut frame)
        .map_err(|_| "MCP stdio input cannot be read".to_owned())?;
    if bytes == 0 {
        return Ok(None);
    }
    if frame.last() == Some(&b'\n') {
        frame.pop();
        if frame.last() == Some(&b'\r') {
            frame.pop();
        }
    }
    if frame.is_empty() || frame.len() > MAX_FRAME_BYTES {
        return Err("MCP stdio frame must contain 1 byte to 1 MiB".to_owned());
    }
    Ok(Some(frame))
}

async fn bounded_response(mut response: reqwest::Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("MCP broker response exceeds 4 MiB".to_owned());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "MCP broker response cannot be read".to_owned())?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("MCP broker response exceeds 4 MiB".to_owned());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn validate_protocol_response(body: &[u8]) -> Result<Vec<u8>, String> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| "MCP broker returned invalid JSON".to_owned())?;
    if !value.is_object() {
        return Err("MCP broker returned a JSON batch".to_owned());
    }
    Ok(body.to_vec())
}

fn extract_sse_response(body: &[u8]) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(body)
        .map_err(|_| "MCP broker returned invalid SSE encoding".to_owned())?;
    let mut found = None;
    for line in text.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.strip_prefix(' ').unwrap_or(data);
        if data.is_empty() {
            continue;
        }
        let data = data.as_bytes().to_vec();
        validate_protocol_response(&data)?;
        if found.replace(data).is_some() {
            return Err("MCP broker returned multiple response events".to_owned());
        }
    }
    found.ok_or_else(|| "MCP broker SSE contained no protocol response".to_owned())
}

fn required_env(env: &impl Fn(&str) -> Option<String>, name: &str) -> Result<String, String> {
    env(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("MCP stdio relay requires the {name} environment variable"))
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::{extract_sse_response, parse_config, read_frame, validate_broker_url};
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::io::Cursor;

    fn valid_env() -> HashMap<String, String> {
        HashMap::from([
            ("AIKS_MCP_AGENT_ID".to_owned(), "agent-1".to_owned()),
            ("AIKS_MCP_GRANT_ID".to_owned(), "grant-1".to_owned()),
            ("AIKS_MCP_GRANT_TOKEN".to_owned(), "a".repeat(64)),
        ])
    }

    #[test]
    fn accepts_only_literal_loopback_broker_urls() {
        for valid in ["http://127.0.0.1:43123/mcp", "http://[::1]:43123/mcp"] {
            assert_eq!(validate_broker_url(valid), Ok(()));
        }
        for invalid in [
            "http://localhost:43123/mcp",
            "http://0.0.0.0:43123/mcp",
            "http://192.168.1.10:43123/mcp",
            "https://127.0.0.1:43123/mcp",
            "http://127.0.0.1/mcp",
            "http://user@127.0.0.1:43123/mcp",
            "http://127.0.0.1:43123/other",
            "http://127.0.0.1:43123/mcp?token=x",
        ] {
            assert!(validate_broker_url(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn accepts_no_token_argument_and_requires_environment_credentials() {
        let args = [
            OsString::from("app"),
            OsString::from("--mcp-stdio-relay"),
            OsString::from("--broker-url"),
            OsString::from("http://127.0.0.1:43123/mcp"),
        ];
        let env = valid_env();
        assert!(parse_config(&args, |name| env.get(name).cloned()).is_ok());

        let mut token_argument = args.to_vec();
        token_argument.push(OsString::from("--grant-token"));
        token_argument.push(OsString::from("secret"));
        assert!(parse_config(&token_argument, |name| env.get(name).cloned()).is_err());
        assert!(parse_config(&args, |_| None).is_err());
    }

    #[test]
    fn bounds_one_newline_delimited_json_frame() {
        let mut reader = Cursor::new(b"{\"jsonrpc\":\"2.0\"}\nnext\n".to_vec());
        assert_eq!(
            read_frame(&mut reader).unwrap().unwrap(),
            br#"{"jsonrpc":"2.0"}"#
        );
        assert_eq!(read_frame(&mut reader).unwrap().unwrap(), b"next");

        let mut oversized = Cursor::new(vec![b'x'; 1024 * 1024 + 1]);
        assert!(read_frame(&mut oversized).is_err());
    }

    #[test]
    fn extracts_exactly_one_bounded_sse_protocol_message() {
        let message = extract_sse_response(
            b"id: 0\nretry: 3000\ndata:\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n",
        )
        .unwrap();
        assert_eq!(message, br#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
        assert!(extract_sse_response(b"event: ping\n\n").is_err());
    }
}
