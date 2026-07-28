use crate::agent_access::authority::AuthorizedRequest;
use crate::agent_access::schema::{AgentGrantSummary, AgentToolDescriptor};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const MAX_CLEANUP_FACTS: usize = 1_000;

pub fn tool_definition(descriptor: AgentToolDescriptor) -> Tool {
    let (description, input_schema) = match descriptor.tool_id {
        "capabilities.read" => (
            "Read the exact tools, scopes, expiry, and limits granted to this Agent.",
            object(json!({"type": "object", "properties": {}, "additionalProperties": false})),
        ),
        "knowledge.read" => (
            "Read one bounded Markdown file relative to an explicitly granted scope.",
            read_schema("Markdown"),
        ),
        "graph.read" => (
            "Read and parse one bounded JSON graph file relative to an explicitly granted scope.",
            read_schema("JSON graph"),
        ),
        "cleanup.suggest" => (
            "Group exact SHA-256 duplicates into review-only suggestions; execution is unavailable.",
            object(json!({
                "type": "object",
                "properties": {
                    "facts": {
                        "type": "array",
                        "maxItems": MAX_CLEANUP_FACTS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "sha256": {"type": "string"},
                                "sizeBytes": {"type": "integer", "minimum": 0},
                                "sourceFormat": {"type": "boolean"}
                            },
                            "required": ["name", "sha256", "sizeBytes", "sourceFormat"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["facts"],
                "additionalProperties": false
            })),
        ),
        "comparison.run" | "classification.propose" => (
            "Reserved semantic-advice adapter. It returns notReady until the governed desktop workflow is connected.",
            object(json!({"type": "object", "additionalProperties": false})),
        ),
        _ => ("Unsupported governed tool.", object(json!({"type": "object"}))),
    };
    let mut tool = Tool::new(descriptor.tool_id, description, input_schema);
    tool.title = Some(descriptor.title.to_owned());
    tool.annotations = Some(
        ToolAnnotations::default()
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    );
    tool
}

pub fn dispatch(
    grant: &AgentGrantSummary,
    authorized: AuthorizedRequest,
    arguments: Option<JsonObject>,
) -> Result<Value, String> {
    let arguments = Value::Object(arguments.unwrap_or_default());
    let result = match authorized.tool.tool_id {
        "capabilities.read" => serde_json::to_value(grant)
            .map_err(|error| format!("Granted capabilities cannot be serialized: {error}"))?,
        "knowledge.read" => {
            let request: ReadRequest = decode(arguments)?;
            let directory = required_directory(authorized.directory)?;
            let text = read_bounded_file(
                &directory,
                &request.relative_path,
                "md",
                authorized.response_budget_bytes,
            )?;
            json!({"scopeId": request.scope_id, "relativePath": request.relative_path, "markdown": text})
        }
        "graph.read" => {
            let request: ReadRequest = decode(arguments)?;
            let directory = required_directory(authorized.directory)?;
            let text = read_bounded_file(
                &directory,
                &request.relative_path,
                "json",
                authorized.response_budget_bytes,
            )?;
            let graph: Value = serde_json::from_str(&text)
                .map_err(|_| "Graph file does not contain valid JSON".to_owned())?;
            json!({"scopeId": request.scope_id, "relativePath": request.relative_path, "graph": graph})
        }
        "cleanup.suggest" => cleanup_suggestions(decode(arguments)?)?,
        "comparison.run" | "classification.propose" => json!({
            "status": "notReady",
            "toolId": authorized.tool.tool_id,
            "executionAvailable": false,
            "message": "The governed desktop adapter is not implemented. No model or classification output was fabricated."
        }),
        _ => return Err("Unsupported governed Agent tool".to_owned()),
    };
    let response_size = serde_json::to_vec(&result)
        .map_err(|error| format!("Tool response cannot be serialized: {error}"))?
        .len() as u64;
    if response_size > authorized.response_budget_bytes {
        return Err("Tool response exceeds the granted response budget".to_owned());
    }
    Ok(result)
}

pub fn requested_scope_id(
    tool_id: &str,
    arguments: &Option<JsonObject>,
) -> Result<Option<String>, String> {
    match tool_id {
        "knowledge.read" | "graph.read" => {
            let request: ReadRequest =
                decode(Value::Object(arguments.clone().unwrap_or_default()))?;
            Ok(Some(request.scope_id))
        }
        _ => Ok(None),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRequest {
    scope_id: String,
    relative_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CleanupRequest {
    facts: Vec<CleanupFact>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CleanupFact {
    name: String,
    sha256: String,
    size_bytes: u64,
    source_format: bool,
}

fn cleanup_suggestions(request: CleanupRequest) -> Result<Value, String> {
    if request.facts.is_empty() || request.facts.len() > MAX_CLEANUP_FACTS {
        return Err("Cleanup facts must contain between 1 and 1000 entries".to_owned());
    }
    let mut groups = BTreeMap::<(String, u64), Vec<CleanupFact>>::new();
    for fact in request.facts {
        validate_name(&fact.name)?;
        if fact.sha256.len() != 64
            || !fact
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("Cleanup SHA-256 must be 64 lowercase hexadecimal characters".to_owned());
        }
        groups
            .entry((fact.sha256.clone(), fact.size_bytes))
            .or_default()
            .push(fact);
    }
    let suggestions = groups
        .into_iter()
        .filter_map(|((sha256, size_bytes), facts)| {
            let retain = facts.iter().find(|fact| fact.source_format)?;
            (facts.len() > 1).then(|| {
                let candidates = facts
                    .iter()
                    .filter(|fact| fact.name != retain.name)
                    .map(|fact| fact.name.clone())
                    .collect::<Vec<_>>();
                json!({
                    "sha256": sha256,
                    "sizeBytes": size_bytes,
                    "retain": retain.name,
                    "reviewCandidates": candidates,
                    "reason": "Exact content identity matches and one source-format original remains."
                })
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "executionAvailable": false,
        "policy": "reviewOnly",
        "suggestions": suggestions
    }))
}

fn read_schema(label: &str) -> Arc<JsonObject> {
    object(json!({
        "type": "object",
        "description": format!("Bounded {label} read"),
        "properties": {
            "scopeId": {"type": "string"},
            "relativePath": {"type": "string"}
        },
        "required": ["scopeId", "relativePath"],
        "additionalProperties": false
    }))
}

fn object(value: Value) -> Arc<JsonObject> {
    Arc::new(
        value
            .as_object()
            .cloned()
            .expect("static schema is an object"),
    )
}

fn decode<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "Tool arguments are invalid".to_owned())
}

fn required_directory(directory: Option<Dir>) -> Result<Dir, String> {
    directory.ok_or_else(|| "Tool requires one granted scope".to_owned())
}

fn read_bounded_file(
    directory: &Dir,
    relative_path: &str,
    expected_extension: &str,
    limit: u64,
) -> Result<String, String> {
    let path = validate_relative_path(relative_path, expected_extension)?;
    reject_symlink_components(directory, &path)?;
    let metadata = directory
        .symlink_metadata(&path)
        .map_err(|_| "Granted file cannot be inspected".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Granted file is not a trusted regular file".to_owned());
    }
    if metadata.len() > limit {
        return Err("Granted file exceeds the response budget".to_owned());
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = directory
        .open_with(&path, &options)
        .map_err(|_| "Granted file cannot be opened without following links".to_owned())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "Granted file cannot be read".to_owned())?;
    if bytes.len() as u64 > limit {
        return Err("Granted file exceeds the response budget".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "Granted file must be UTF-8".to_owned())
}

fn validate_relative_path(value: &str, expected_extension: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("Relative path is invalid".to_owned());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.extension().and_then(|extension| extension.to_str()) != Some(expected_extension)
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "Tool requires a normalized relative .{expected_extension} path"
        ));
    }
    Ok(path.to_path_buf())
}

fn reject_symlink_components(directory: &Dir, path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err("Relative path is invalid".to_owned());
        };
        current.push(value);
        let metadata = directory
            .symlink_metadata(&current)
            .map_err(|_| "Granted path component cannot be inspected".to_owned())?;
        if metadata.file_type().is_symlink() {
            return Err("Granted path contains a symbolic link".to_owned());
        }
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().count() > 512 || value.chars().any(char::is_control) {
        return Err("Cleanup fact name is invalid".to_owned());
    }
    Ok(())
}
