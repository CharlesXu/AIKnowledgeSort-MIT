use super::config::ModelConfigSummary;
use super::protocol::{AgentAdjudication, ComparisonEnvelope, ModelProposal};
use super::ModelTransport;
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;
use std::time::Duration;

const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const USER_AGENT: &str = "AIKnowledgeSort/0.1";
const PROPOSAL_SYSTEM_PROMPT: &str = "Return one JSON object matching the supplied ModelProposal schema. Use only the supplied envelope and evidence IDs. Do not propose or invoke filesystem, archive, knowledge, or graph mutations.";
const ADJUDICATION_SYSTEM_PROMPT: &str = "Act as the Agent-side adjudicator. Return one JSON object matching the supplied AgentAdjudication schema. Compare both proposals only against the identical supplied envelope. Require cited evidence and choose review when support or agreement is insufficient. Do not authorize any operation.";

pub struct OpenAiCompatibleTransport;

impl ModelTransport for OpenAiCompatibleTransport {
    fn propose(
        &self,
        config: &ModelConfigSummary,
        envelope_json: &[u8],
    ) -> Result<ModelProposal, String> {
        let envelope_text = String::from_utf8(envelope_json.to_vec())
            .map_err(|_| "Comparison envelope is not valid UTF-8".to_owned())?;
        let body = chat_request(config, PROPOSAL_SYSTEM_PROMPT, envelope_text);
        let bytes = execute(config, &body)?;
        let proposal = parse_proposal_response(&bytes)?;
        let envelope: ComparisonEnvelope = serde_json::from_slice(envelope_json)
            .map_err(|error| format!("Comparison envelope is invalid: {error}"))?;
        proposal.validate(&envelope)?;
        Ok(proposal)
    }

    fn adjudicate(
        &self,
        config: &ModelConfigSummary,
        envelope_json: &[u8],
        desktop: &ModelProposal,
        agent: &ModelProposal,
    ) -> Result<AgentAdjudication, String> {
        let envelope: ComparisonEnvelope = serde_json::from_slice(envelope_json)
            .map_err(|error| format!("Comparison envelope is invalid: {error}"))?;
        let payload = AdjudicationPayload {
            envelope: &envelope,
            desktop_proposal: desktop,
            agent_proposal: agent,
        };
        let user_content = serde_json::to_string(&payload)
            .map_err(|error| format!("Adjudication request cannot be serialized: {error}"))?;
        let body = chat_request(config, ADJUDICATION_SYSTEM_PROMPT, user_content);
        let bytes = execute(config, &body)?;
        let adjudication = parse_adjudication_response(&bytes)?;
        adjudication.validate(&envelope)?;
        Ok(adjudication)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdjudicationPayload<'a> {
    envelope: &'a ComparisonEnvelope,
    desktop_proposal: &'a ModelProposal,
    agent_proposal: &'a ModelProposal,
}

fn chat_request(
    config: &ModelConfigSummary,
    system_content: &str,
    user_content: String,
) -> serde_json::Value {
    json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": system_content},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0,
        "response_format": {"type": "json_object"}
    })
}

fn execute(config: &ModelConfigSummary, body: &serde_json::Value) -> Result<Vec<u8>, String> {
    let timeout = Duration::from_millis(config.timeout_ms);
    let client = Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("Model HTTP client cannot be built: {error}"))?;
    let mut request = client.post(&config.endpoint_url).json(body);
    if config.authenticated {
        let environment = config.credential_environment.as_deref().ok_or_else(|| {
            "Authenticated model configuration has no credential reference".to_owned()
        })?;
        let credential = std::env::var(environment)
            .map_err(|_| format!("Model credential environment {environment} is not set"))?;
        request = request.bearer_auth(credential);
    }
    let response = request
        .send()
        .map_err(|error| format!("Model request failed: {error}"))?;
    read_success_response(response)
}

fn read_success_response(mut response: Response) -> Result<Vec<u8>, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Model endpoint returned HTTP {status}"));
    }
    let content_length = response.content_length();
    read_bounded(&mut response, content_length)
}

fn read_bounded(reader: &mut impl Read, declared_length: Option<u64>) -> Result<Vec<u8>, String> {
    if declared_length.is_some_and(|length| length > MAX_RESPONSE_BYTES as u64) {
        return Err("Model response exceeds 256 KiB".to_owned());
    }
    let mut bytes =
        Vec::with_capacity(declared_length.unwrap_or(0).min(MAX_RESPONSE_BYTES as u64) as usize);
    reader
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Model response cannot be read: {error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("Model response exceeds 256 KiB".to_owned());
    }
    Ok(bytes)
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

fn response_content(bytes: &[u8]) -> Result<String, String> {
    let response: ChatCompletionResponse = serde_json::from_slice(bytes)
        .map_err(|error| format!("Model response JSON is invalid: {error}"))?;
    if response.choices.len() != 1 || response.choices[0].message.content.trim().is_empty() {
        return Err("Model response must contain one non-empty choice".to_owned());
    }
    Ok(response
        .choices
        .into_iter()
        .next()
        .expect("one choice")
        .message
        .content)
}

pub(crate) fn complete_json(
    config: &ModelConfigSummary,
    system_prompt: &str,
    user_json: &str,
) -> Result<String, String> {
    let body = chat_request(config, system_prompt, user_json.to_owned());
    let bytes = execute(config, &body)?;
    response_content(&bytes)
}

pub(crate) fn parse_proposal_response(bytes: &[u8]) -> Result<ModelProposal, String> {
    serde_json::from_str(&response_content(bytes)?)
        .map_err(|error| format!("Model proposal JSON is invalid: {error}"))
}

fn parse_adjudication_response(bytes: &[u8]) -> Result<AgentAdjudication, String> {
    serde_json::from_str(&response_content(bytes)?)
        .map_err(|error| format!("Agent adjudication JSON is invalid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        execute, parse_adjudication_response, parse_proposal_response, read_bounded,
        MAX_RESPONSE_BYTES,
    };
    use crate::model_runtime::config::{ModelConfigSummary, ModelLocation};
    use std::io::{Cursor, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn rejects_markdown_fenced_provider_content() {
        assert!(parse_proposal_response(
            br#"{"choices":[{"message":{"content":"```json\\n{}\\n```"}}]}"#,
        )
        .is_err());
    }

    #[test]
    fn rejects_missing_malformed_unknown_and_oversized_content() {
        assert!(parse_proposal_response(br#"{}"#).is_err());
        assert!(parse_proposal_response(br#"{"choices":[]}"#).is_err());
        assert!(
            parse_proposal_response(br#"{"choices":[{"message":{"content":"not-json"}}]}"#)
                .is_err()
        );
        let unknown = serde_json::json!({
            "choices": [{"message": {"content": "{\"summary\":\"x\",\"relations\":[],\"unknown\":true}"}}]
        });
        assert!(parse_proposal_response(&serde_json::to_vec(&unknown).unwrap()).is_err());

        let missing_reason = serde_json::json!({
            "choices": [{"message": {"content": "{\"decision\":\"review\",\"evidenceIds\":[\"line-1-1\"],\"selectedSide\":null,\"revisedRelations\":[]}"}}]
        });
        assert!(
            parse_adjudication_response(&serde_json::to_vec(&missing_reason).unwrap()).is_err()
        );

        let oversized = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        assert!(read_bounded(&mut Cursor::new(oversized), None).is_err());
        assert!(
            read_bounded(&mut Cursor::new(b"{}"), Some(MAX_RESPONSE_BYTES as u64 + 1)).is_err()
        );
    }

    fn config(endpoint_url: String, timeout_ms: u64) -> ModelConfigSummary {
        ModelConfigSummary {
            config_id: "local-test".to_owned(),
            label: "Local test".to_owned(),
            location: ModelLocation::Local,
            endpoint_url,
            model: "test-model".to_owned(),
            timeout_ms,
            authenticated: false,
            credential_environment: None,
        }
    }

    fn serve(response: String, delay: Duration) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("read test server address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept model request");
            thread::sleep(delay);
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{address}/v1/chat/completions"), handle)
    }

    #[test]
    fn rejects_non_success_declared_overflow_and_timeout() {
        let (endpoint, server) = serve(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n".to_owned(),
            Duration::ZERO,
        );
        assert!(execute(&config(endpoint, 1_000), &serde_json::json!({})).is_err());
        server.join().expect("join non-success server");

        let (endpoint, server) = serve(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                MAX_RESPONSE_BYTES + 1
            ),
            Duration::ZERO,
        );
        assert!(execute(&config(endpoint, 1_000), &serde_json::json!({})).is_err());
        server.join().expect("join overflow server");

        let (endpoint, server) = serve(
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}".to_owned(),
            Duration::from_millis(100),
        );
        assert!(execute(&config(endpoint, 20), &serde_json::json!({})).is_err());
        server.join().expect("join timeout server");
    }
}
