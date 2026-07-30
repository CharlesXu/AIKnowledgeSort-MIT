use super::config::{
    validate_api_key, validate_endpoint, ModelCredentialSource, ModelLocation, ModelProtocol,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use url::Url;

const MAX_MODELS: usize = 1_024;
const USER_AGENT: &str = "AIKnowledgeSort/0.1";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverModelsRequest {
    pub config_id: Option<String>,
    pub location: ModelLocation,
    pub endpoint_url: String,
    pub timeout_ms: u64,
    pub authenticated: bool,
    pub credential_source: ModelCredentialSource,
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModels {
    pub provider_protocol: ModelProtocol,
    pub models_endpoint_url: String,
    pub completion_endpoint_url: String,
    pub models: Vec<String>,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelRecord>,
}

#[derive(Deserialize)]
struct ModelRecord {
    id: String,
    #[serde(default)]
    object: Option<String>,
    #[serde(default, rename = "type")]
    record_type: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
}

pub(crate) fn discover(
    request: &DiscoverModelsRequest,
    credential: Option<&str>,
) -> Result<DiscoveredModels, String> {
    validate_endpoint(request.location, &request.endpoint_url)?;
    if !(1_000..=120_000).contains(&request.timeout_ms) {
        return Err("Model timeout must be between 1 and 120 seconds".to_owned());
    }
    if request.authenticated && credential.is_none() {
        return Err("Authenticated model discovery requires an API key".to_owned());
    }
    if let Some(value) = credential {
        validate_api_key(value)?;
    }

    let timeout = Duration::from_millis(request.timeout_ms);
    let client = Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("Model HTTP client cannot be built: {error}"))?;
    let mut failures = Vec::new();
    for endpoint in model_endpoint_candidates(&request.endpoint_url)? {
        validate_endpoint(request.location, endpoint.as_str())?;
        let mut call = client.get(endpoint.clone());
        if let Some(api_key) = credential {
            call = call
                .bearer_auth(api_key)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01");
        }
        match call.send() {
            Ok(response) if response.status().is_success() => {
                match super::openai_compatible::read_success_response(response) {
                    Ok(bytes) => match parse_models(&endpoint, &bytes) {
                        Ok(discovered) => return Ok(discovered),
                        Err(error) => failures.push(format!("{}: {error}", endpoint.path())),
                    },
                    Err(error) => failures.push(format!("{}: {error}", endpoint.path())),
                }
            }
            Ok(response) => failures.push(format!(
                "{} returned HTTP {}",
                endpoint.path(),
                response.status()
            )),
            Err(error) => failures.push(format!("{} failed: {error}", endpoint.path())),
        }
    }
    Err(format!(
        "No supported model list endpoint was found: {}",
        failures.join("; ")
    ))
}

fn parse_models(endpoint: &Url, bytes: &[u8]) -> Result<DiscoveredModels, String> {
    let response: ModelsResponse = serde_json::from_slice(bytes)
        .map_err(|error| format!("Model list JSON is invalid: {error}"))?;
    if response.data.len() > MAX_MODELS {
        return Err("Model list exceeds 1024 entries".to_owned());
    }
    let provider_protocol = infer_protocol(&response.data)?;
    let mut models = response
        .data
        .into_iter()
        .map(|record| validate_model_id(record.id))
        .collect::<Result<Vec<_>, _>>()?;
    models.sort();
    models.dedup();
    if models.is_empty() {
        return Err("Model list is empty".to_owned());
    }
    Ok(DiscoveredModels {
        provider_protocol,
        models_endpoint_url: endpoint.as_str().to_owned(),
        completion_endpoint_url: completion_endpoint(endpoint, provider_protocol)?,
        models,
    })
}

fn infer_protocol(records: &[ModelRecord]) -> Result<ModelProtocol, String> {
    let has_anthropic = records.iter().any(|record| {
        record.record_type.as_deref() == Some("model") || record.display_name.is_some()
    });
    let has_open_ai = records
        .iter()
        .any(|record| record.object.as_deref() == Some("model"));
    match (has_open_ai, has_anthropic) {
        (true, false) => Ok(ModelProtocol::OpenAi),
        (false, true) => Ok(ModelProtocol::Anthropic),
        (true, true) => Err("Model list contains mixed provider protocol markers".to_owned()),
        (false, false) => Ok(ModelProtocol::OpenAi),
    }
}

fn validate_model_id(id: String) -> Result<String, String> {
    if id.is_empty()
        || id.trim() != id
        || id.chars().count() > 256
        || id.chars().any(char::is_control)
    {
        return Err("Model IDs must contain 1 to 256 visible characters".to_owned());
    }
    Ok(id)
}

fn model_endpoint_candidates(endpoint_url: &str) -> Result<Vec<Url>, String> {
    let endpoint =
        Url::parse(endpoint_url).map_err(|_| "Model endpoint URL is invalid".to_owned())?;
    let path = endpoint.path().trim_end_matches('/');
    let paths = if let Some(prefix) = path.strip_suffix("/chat/completions") {
        vec![format!("{prefix}/models")]
    } else if let Some(prefix) = path.strip_suffix("/messages") {
        vec![format!("{prefix}/models")]
    } else if path.ends_with("/models") {
        vec![path.to_owned()]
    } else if path.ends_with("/v1") {
        vec![format!("{path}/models")]
    } else {
        vec![format!("{path}/v1/models"), format!("{path}/models")]
    };
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .map(|path| with_path(&endpoint, &path))
        .collect()
}

fn completion_endpoint(models_endpoint: &Url, protocol: ModelProtocol) -> Result<String, String> {
    let path = models_endpoint.path().trim_end_matches('/');
    let prefix = path
        .strip_suffix("/models")
        .ok_or_else(|| "Model list endpoint must end with /models".to_owned())?;
    let suffix = match protocol {
        ModelProtocol::OpenAi => "chat/completions",
        ModelProtocol::Anthropic => "messages",
    };
    Ok(with_path(models_endpoint, &format!("{prefix}/{suffix}"))?
        .as_str()
        .to_owned())
}

pub(crate) fn normalize_completion_endpoint(
    endpoint_url: &str,
    protocol: ModelProtocol,
) -> Result<String, String> {
    let endpoint =
        Url::parse(endpoint_url).map_err(|_| "Model endpoint URL is invalid".to_owned())?;
    let path = endpoint.path().trim_end_matches('/');
    let suffix = match protocol {
        ModelProtocol::OpenAi => "chat/completions",
        ModelProtocol::Anthropic => "messages",
    };
    let path = if let Some(prefix) = path.strip_suffix("/models") {
        format!("{prefix}/{suffix}")
    } else if let Some(prefix) = path.strip_suffix("/chat/completions") {
        format!("{prefix}/{suffix}")
    } else if let Some(prefix) = path.strip_suffix("/messages") {
        format!("{prefix}/{suffix}")
    } else if path.ends_with("/v1") {
        format!("{path}/{suffix}")
    } else {
        format!("{path}/v1/{suffix}")
    };
    Ok(with_path(&endpoint, &path)?.as_str().to_owned())
}

fn with_path(endpoint: &Url, path: &str) -> Result<Url, String> {
    let mut normalized = endpoint.clone();
    normalized.set_path(path);
    normalized.set_query(None);
    normalized.set_fragment(None);
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        discover, model_endpoint_candidates, normalize_completion_endpoint, parse_models,
        DiscoverModelsRequest,
    };
    use crate::model_runtime::config::{ModelCredentialSource, ModelLocation, ModelProtocol};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use url::Url;

    #[test]
    fn derives_model_endpoints_from_base_v1_models_and_complete_urls() {
        let cases = [
            ("https://api.example.com", &["/v1/models", "/models"][..]),
            ("https://api.example.com/", &["/v1/models", "/models"][..]),
            ("https://api.example.com/v1", &["/v1/models"][..]),
            ("https://api.example.com/chat/completions", &["/models"][..]),
            (
                "https://api.example.com/openai/v1/chat/completions",
                &["/openai/v1/models"][..],
            ),
            ("https://api.example.com/messages", &["/models"][..]),
            (
                "https://api.example.com/anthropic/v1/messages",
                &["/anthropic/v1/models"][..],
            ),
            ("https://api.example.com/models", &["/models"][..]),
            ("https://api.example.com/v1/models/", &["/v1/models"][..]),
            (
                "https://api.example.com/gateway",
                &["/gateway/v1/models", "/gateway/models"][..],
            ),
        ];
        for (input, expected_paths) in cases {
            assert_eq!(
                model_endpoint_candidates(input)
                    .unwrap()
                    .iter()
                    .map(Url::path)
                    .collect::<Vec<_>>(),
                expected_paths,
                "unexpected discovery candidates for {input}",
            );
        }
    }

    #[test]
    fn normalizes_all_supported_url_shapes_for_both_protocols() {
        let cases = [
            (
                "https://api.example.com",
                ModelProtocol::OpenAi,
                "https://api.example.com/v1/chat/completions",
            ),
            (
                "https://api.example.com/v1",
                ModelProtocol::Anthropic,
                "https://api.example.com/v1/messages",
            ),
            (
                "https://api.example.com/models",
                ModelProtocol::OpenAi,
                "https://api.example.com/chat/completions",
            ),
            (
                "https://api.example.com/v1/models/",
                ModelProtocol::Anthropic,
                "https://api.example.com/v1/messages",
            ),
            (
                "https://api.example.com/openai/v1/chat/completions/",
                ModelProtocol::OpenAi,
                "https://api.example.com/openai/v1/chat/completions",
            ),
            (
                "https://api.example.com/anthropic/v1/messages/",
                ModelProtocol::Anthropic,
                "https://api.example.com/anthropic/v1/messages",
            ),
        ];
        for (input, protocol, expected) in cases {
            assert_eq!(
                normalize_completion_endpoint(input, protocol).unwrap(),
                expected,
                "unexpected completion endpoint for {input}",
            );
        }
    }

    #[test]
    fn identifies_openai_and_anthropic_lists_and_normalizes_completion_urls() {
        let open_ai = parse_models(
            &Url::parse("https://api.example.com/v1/models").unwrap(),
            br#"{"data":[{"id":"gpt-5","object":"model"},{"id":"gpt-4.1","object":"model"}]}"#,
        )
        .unwrap();
        assert_eq!(open_ai.provider_protocol, ModelProtocol::OpenAi);
        assert_eq!(
            open_ai.completion_endpoint_url,
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(open_ai.models, ["gpt-4.1", "gpt-5"]);

        let anthropic = parse_models(
            &Url::parse("https://api.example.com/v1/models").unwrap(),
            br#"{"data":[{"id":"claude-sonnet-4-5","type":"model","display_name":"Claude Sonnet 4.5"}]}"#,
        )
        .unwrap();
        assert_eq!(anthropic.provider_protocol, ModelProtocol::Anthropic);
        assert_eq!(
            anthropic.completion_endpoint_url,
            "https://api.example.com/v1/messages"
        );
        assert_eq!(
            normalize_completion_endpoint("https://api.example.com", ModelProtocol::Anthropic)
                .unwrap(),
            "https://api.example.com/v1/messages"
        );
    }

    #[test]
    fn probes_a_base_url_and_returns_the_detected_anthropic_protocol() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind discovery server");
        let address = listener.local_addr().expect("read discovery address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept discovery request");
            let mut request = [0_u8; 2_048];
            let length = stream.read(&mut request).expect("read discovery request");
            let request = String::from_utf8_lossy(&request[..length]);
            assert!(request.starts_with("GET /v1/models "));
            let body =
                r#"{"data":[{"id":"claude-test","type":"model","display_name":"Claude Test"}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("write discovery response");
        });

        let result = discover(
            &DiscoverModelsRequest {
                config_id: None,
                location: ModelLocation::Local,
                endpoint_url: format!("http://{address}"),
                timeout_ms: 1_000,
                authenticated: false,
                credential_source: ModelCredentialSource::Environment,
                api_key: None,
            },
            None,
        )
        .expect("discover Anthropic model");
        server.join().expect("join discovery server");
        assert_eq!(result.provider_protocol, ModelProtocol::Anthropic);
        assert_eq!(result.models, ["claude-test"]);
        assert!(result.completion_endpoint_url.ends_with("/v1/messages"));
    }

    #[test]
    fn falls_back_to_a_model_endpoint_without_v1() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind discovery server");
        let address = listener.local_addr().expect("read discovery address");
        let server = thread::spawn(move || {
            for (expected_path, status, body) in [
                (
                    "/gateway/v1/models",
                    "404 Not Found",
                    r#"{"error":"missing"}"#,
                ),
                (
                    "/gateway/models",
                    "200 OK",
                    r#"{"data":[{"id":"local-chat","object":"model"}]}"#,
                ),
            ] {
                let (mut stream, _) = listener.accept().expect("accept discovery request");
                let mut request = [0_u8; 2_048];
                let length = stream.read(&mut request).expect("read discovery request");
                let request = String::from_utf8_lossy(&request[..length]);
                assert!(
                    request.starts_with(&format!("GET {expected_path} ")),
                    "unexpected request: {request}",
                );
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                )
                .expect("write discovery response");
            }
        });

        let result = discover(
            &DiscoverModelsRequest {
                config_id: None,
                location: ModelLocation::Local,
                endpoint_url: format!("http://{address}/gateway"),
                timeout_ms: 1_000,
                authenticated: false,
                credential_source: ModelCredentialSource::Environment,
                api_key: None,
            },
            None,
        )
        .expect("discover model without v1");
        server.join().expect("join discovery server");
        assert_eq!(result.provider_protocol, ModelProtocol::OpenAi);
        assert_eq!(result.models, ["local-chat"]);
        assert!(result.models_endpoint_url.ends_with("/gateway/models"));
        assert!(result
            .completion_endpoint_url
            .ends_with("/gateway/chat/completions"));
    }
}
