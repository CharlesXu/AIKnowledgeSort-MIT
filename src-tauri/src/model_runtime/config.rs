use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use url::{Host, Url};

const CONFIG_FILENAME: &str = "model-runtime-v1.json";
const CONFIG_SCHEMA_VERSION: u32 = 1;
const MAX_CONFIGS: usize = 32;
const MAX_CONFIG_BYTES: u64 = 128 * 1024;
const MAX_CONFIG_ID_CHARS: usize = 64;
const MAX_TEXT_CHARS: usize = 256;
const MAX_ENDPOINT_BYTES: usize = 2 * 1024;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelLocation {
    Local,
    Remote,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigInput {
    pub config_id: String,
    pub label: String,
    pub location: ModelLocation,
    pub endpoint_url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub authenticated: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigSummary {
    pub config_id: String,
    pub label: String,
    pub location: ModelLocation,
    pub endpoint_url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub authenticated: bool,
    pub credential_environment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuntimeState {
    pub schema_version: u32,
    pub configs: Vec<ModelConfigSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedModelRuntime {
    schema_version: u32,
    configs: Vec<ModelConfigInput>,
}

pub struct ModelConfigStore {
    directory: PathBuf,
}

impl ModelConfigStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn inspect(&self) -> Result<ModelRuntimeState, String> {
        let persisted = self.read()?;
        state_from_persisted(persisted)
    }

    pub fn upsert(&self, input: ModelConfigInput) -> Result<ModelRuntimeState, String> {
        validate_input(&input)?;
        let mut persisted = self.read()?;
        if let Some(index) = persisted
            .configs
            .iter()
            .position(|config| config.config_id == input.config_id)
        {
            persisted.configs[index] = input;
        } else {
            if persisted.configs.len() >= MAX_CONFIGS {
                return Err("At most 32 model configurations are allowed".to_owned());
            }
            persisted.configs.push(input);
        }
        persisted
            .configs
            .sort_by(|left, right| left.config_id.cmp(&right.config_id));
        validate_persisted(&persisted)?;
        self.write(&persisted)?;
        state_from_persisted(persisted)
    }

    pub fn remove(&self, config_id: &str) -> Result<ModelRuntimeState, String> {
        validate_config_id(config_id)?;
        let mut persisted = self.read()?;
        let previous_len = persisted.configs.len();
        persisted
            .configs
            .retain(|config| config.config_id != config_id);
        if persisted.configs.len() == previous_len {
            return Err("Model configuration does not exist".to_owned());
        }
        self.write(&persisted)?;
        state_from_persisted(persisted)
    }

    pub fn get(&self, config_id: &str) -> Result<ModelConfigSummary, String> {
        validate_config_id(config_id)?;
        self.inspect()?
            .configs
            .into_iter()
            .find(|config| config.config_id == config_id)
            .ok_or_else(|| "Model configuration does not exist".to_owned())
    }

    fn read(&self) -> Result<PersistedModelRuntime, String> {
        ensure_config_directory(&self.directory)?;
        let path = self.directory.join(CONFIG_FILENAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PersistedModelRuntime {
                    schema_version: CONFIG_SCHEMA_VERSION,
                    configs: Vec::new(),
                })
            }
            Err(error) => return Err(format!("Model configuration cannot be inspected: {error}")),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("Model configuration is not a regular file".to_owned());
        }
        if metadata.len() > MAX_CONFIG_BYTES {
            return Err("Model configuration exceeds 128 KiB".to_owned());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        File::open(&path)
            .and_then(|file| file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|error| format!("Model configuration cannot be read: {error}"))?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err("Model configuration exceeds 128 KiB".to_owned());
        }
        let persisted: PersistedModelRuntime = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Model configuration JSON is invalid: {error}"))?;
        validate_persisted(&persisted)?;
        Ok(persisted)
    }

    fn write(&self, persisted: &PersistedModelRuntime) -> Result<(), String> {
        ensure_config_directory(&self.directory)?;
        validate_existing_destination(&self.directory.join(CONFIG_FILENAME))?;
        let bytes = serde_json::to_vec_pretty(persisted)
            .map_err(|error| format!("Model configuration cannot be serialized: {error}"))?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err("Model configuration exceeds 128 KiB".to_owned());
        }
        let temporary_path = self
            .directory
            .join(format!(".model-runtime-v1-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut temporary = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)
                .map_err(|error| {
                    format!("Temporary model configuration cannot be created: {error}")
                })?;
            temporary
                .write_all(&bytes)
                .and_then(|_| temporary.sync_all())
                .map_err(|error| {
                    format!("Temporary model configuration cannot be synced: {error}")
                })?;
            fs::rename(&temporary_path, self.directory.join(CONFIG_FILENAME))
                .map_err(|error| format!("Model configuration cannot be replaced: {error}"))?;
            File::open(&self.directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("Model configuration directory cannot be synced: {error}"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        result
    }
}

fn state_from_persisted(persisted: PersistedModelRuntime) -> Result<ModelRuntimeState, String> {
    validate_persisted(&persisted)?;
    Ok(ModelRuntimeState {
        schema_version: persisted.schema_version,
        configs: persisted
            .configs
            .into_iter()
            .map(summary_from_input)
            .collect(),
    })
}

fn summary_from_input(input: ModelConfigInput) -> ModelConfigSummary {
    let credential_environment = input
        .authenticated
        .then(|| credential_environment(&input.config_id));
    ModelConfigSummary {
        config_id: input.config_id,
        label: input.label,
        location: input.location,
        endpoint_url: input.endpoint_url,
        model: input.model,
        timeout_ms: input.timeout_ms,
        authenticated: input.authenticated,
        credential_environment,
    }
}

fn validate_persisted(persisted: &PersistedModelRuntime) -> Result<(), String> {
    if persisted.schema_version != CONFIG_SCHEMA_VERSION {
        return Err("Model configuration schema version is unsupported".to_owned());
    }
    if persisted.configs.len() > MAX_CONFIGS {
        return Err("At most 32 model configurations are allowed".to_owned());
    }
    let mut ids = HashSet::with_capacity(persisted.configs.len());
    for config in &persisted.configs {
        validate_input(config)?;
        if !ids.insert(config.config_id.as_str()) {
            return Err("Model configuration IDs must be unique".to_owned());
        }
    }
    Ok(())
}

fn validate_input(input: &ModelConfigInput) -> Result<(), String> {
    validate_config_id(&input.config_id)?;
    validate_visible_text(&input.label, "Model configuration label")?;
    validate_visible_text(&input.model, "Model name")?;
    if input.endpoint_url.is_empty() || input.endpoint_url.len() > MAX_ENDPOINT_BYTES {
        return Err("Model endpoint must be between 1 byte and 2 KiB".to_owned());
    }
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&input.timeout_ms) {
        return Err("Model timeout must be between 1 and 120 seconds".to_owned());
    }
    let endpoint =
        Url::parse(&input.endpoint_url).map_err(|_| "Model endpoint URL is invalid".to_owned())?;
    if !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err("Model endpoint cannot contain credentials, query, or fragment".to_owned());
    }
    let host = endpoint
        .host()
        .ok_or_else(|| "Model endpoint must include a host".to_owned())?;
    match input.location {
        ModelLocation::Local => {
            if endpoint.scheme() != "http" || !is_literal_loopback(host) {
                return Err(
                    "Local model endpoints must use HTTP and a literal loopback IP".to_owned(),
                );
            }
        }
        ModelLocation::Remote => {
            if endpoint.scheme() != "https" || is_disallowed_remote_host(host) {
                return Err(
                    "Remote model endpoints must use HTTPS and a non-private host".to_owned(),
                );
            }
        }
    }
    Ok(())
}

fn validate_config_id(config_id: &str) -> Result<(), String> {
    if config_id.is_empty()
        || config_id.chars().count() > MAX_CONFIG_ID_CHARS
        || !config_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(
            "Model configuration ID must use lowercase ASCII letters, digits, and hyphens"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_visible_text(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > MAX_TEXT_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(format!("{field} must contain 1 to 256 visible characters"));
    }
    Ok(())
}

fn credential_environment(config_id: &str) -> String {
    let suffix = config_id
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' => (byte - b'a' + b'A') as char,
            b'0'..=b'9' => byte as char,
            b'-' => '_',
            _ => unreachable!("validated config ID"),
        })
        .collect::<String>();
    format!("AIKS_MODEL_API_KEY_{suffix}")
}

fn is_literal_loopback(host: Host<&str>) -> bool {
    match host {
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
        Host::Domain(_) => false,
    }
}

fn is_disallowed_remote_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => is_disallowed_remote_ipv4(address),
        Host::Ipv6(address) => is_disallowed_remote_ipv6(address),
    }
}

fn is_disallowed_remote_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_unspecified()
        || address.is_multicast()
        || first == 0
        || first >= 224
        || (first == 100 && (64..=127).contains(&second))
        || (first == 192 && second == 0)
        || (first == 198 && (second == 18 || second == 19))
}

fn is_disallowed_remote_ipv6(address: Ipv6Addr) -> bool {
    let first = address.segments()[0];
    address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || matches!(IpAddr::V6(address).to_canonical(), IpAddr::V4(ipv4) if is_disallowed_remote_ipv4(ipv4))
}

fn ensure_config_directory(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("Model configuration directory cannot be created: {error}"))?;
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("Model configuration directory cannot be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Model configuration directory is not a regular directory".to_owned());
    }
    Ok(())
}

fn validate_existing_destination(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Model configuration is not a regular file".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Model configuration cannot be inspected: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelConfigInput, ModelConfigStore, ModelLocation};
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ai-knowledge-sort-model-config-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("create model config test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn local_config() -> ModelConfigInput {
        ModelConfigInput {
            config_id: "local-ollama".to_owned(),
            label: "Local Ollama".to_owned(),
            location: ModelLocation::Local,
            endpoint_url: "http://127.0.0.1:11434/v1/chat/completions".to_owned(),
            model: "qwen3:8b".to_owned(),
            timeout_ms: 30_000,
            authenticated: false,
        }
    }

    fn remote_config() -> ModelConfigInput {
        ModelConfigInput {
            config_id: "remote-reasoner".to_owned(),
            label: "Remote Reasoner".to_owned(),
            location: ModelLocation::Remote,
            endpoint_url: "https://models.example.com/v1/chat/completions".to_owned(),
            model: "reasoner-v1".to_owned(),
            timeout_ms: 60_000,
            authenticated: true,
        }
    }

    #[test]
    fn upsert_inspect_and_remove_round_trip_without_secret_values() {
        let directory = TestDirectory::new();
        let store = ModelConfigStore::new(directory.path().to_owned());

        let local_state = store.upsert(local_config()).expect("upsert local config");
        assert_eq!(local_state.schema_version, 1);
        assert_eq!(local_state.configs.len(), 1);
        assert_eq!(local_state.configs[0].credential_environment, None);

        let remote_state = store.upsert(remote_config()).expect("upsert remote config");
        assert_eq!(
            remote_state.configs[1].credential_environment.as_deref(),
            Some("AIKS_MODEL_API_KEY_REMOTE_REASONER")
        );
        let persisted = fs::read_to_string(directory.path().join("model-runtime-v1.json"))
            .expect("read persisted model configs");
        assert!(!persisted.contains("apiKey"));
        assert!(!persisted.contains("secret"));

        let inspected = store.inspect().expect("inspect model configs");
        assert_eq!(inspected, remote_state);
        let removed = store.remove("local-ollama").expect("remove local config");
        assert_eq!(removed.configs.len(), 1);
        assert_eq!(removed.configs[0].config_id, "remote-reasoner");
    }

    #[test]
    fn rejects_invalid_fields_endpoints_and_timeouts() {
        let directory = TestDirectory::new();
        let store = ModelConfigStore::new(directory.path().to_owned());
        let invalid = [
            (
                "invalid id".to_owned(),
                "http://127.0.0.1:11434/v1".to_owned(),
                30_000,
            ),
            (
                "local-ok".to_owned(),
                "http://192.168.1.2:11434/v1".to_owned(),
                30_000,
            ),
            (
                "local-ok".to_owned(),
                "https://127.0.0.1:11434/v1".to_owned(),
                30_000,
            ),
            (
                "local-ok".to_owned(),
                "http://user:pass@127.0.0.1:11434/v1".to_owned(),
                30_000,
            ),
            (
                "local-ok".to_owned(),
                "http://127.0.0.1:11434/v1?q=1".to_owned(),
                30_000,
            ),
            (
                "local-ok".to_owned(),
                "http://127.0.0.1:11434/v1#fragment".to_owned(),
                30_000,
            ),
            (
                "local-ok".to_owned(),
                "http://127.0.0.1:11434/v1".to_owned(),
                999,
            ),
            (
                "local-ok".to_owned(),
                "http://127.0.0.1:11434/v1".to_owned(),
                120_001,
            ),
        ];
        for (config_id, endpoint_url, timeout_ms) in invalid {
            let mut config = local_config();
            config.config_id = config_id;
            config.endpoint_url = endpoint_url;
            config.timeout_ms = timeout_ms;
            assert!(store.upsert(config).is_err());
        }

        for endpoint_url in [
            "http://models.example.com/v1/chat/completions",
            "https://localhost/v1/chat/completions",
            "https://127.0.0.1/v1/chat/completions",
            "https://10.0.0.1/v1/chat/completions",
            "https://172.16.0.1/v1/chat/completions",
            "https://192.168.1.2/v1/chat/completions",
        ] {
            let mut config = remote_config();
            config.endpoint_url = endpoint_url.to_owned();
            assert!(store.upsert(config).is_err(), "accepted {endpoint_url}");
        }
    }

    #[test]
    fn rejects_control_characters_and_oversized_values() {
        let directory = TestDirectory::new();
        let store = ModelConfigStore::new(directory.path().to_owned());
        let mut invalid = Vec::new();
        let mut control_character = local_config();
        control_character.label = "bad\nlabel".to_owned();
        invalid.push(control_character);
        let mut oversized_model = local_config();
        oversized_model.model = "x".repeat(257);
        invalid.push(oversized_model);
        let mut oversized_url = local_config();
        oversized_url.endpoint_url = format!("http://127.0.0.1/{}", "x".repeat(2049));
        invalid.push(oversized_url);

        for config in invalid {
            assert!(store.upsert(config).is_err());
        }
    }

    #[test]
    fn rejects_unknown_json_fields_duplicate_ids_and_more_than_32_configs() {
        let directory = TestDirectory::new();
        let path = directory.path().join("model-runtime-v1.json");
        fs::write(
            &path,
            r#"{"schemaVersion":1,"configs":[],"unexpected":true}"#,
        )
        .expect("write unknown field fixture");
        assert!(ModelConfigStore::new(directory.path().to_owned())
            .inspect()
            .is_err());

        fs::write(
            &path,
            r#"{"schemaVersion":1,"configs":[{"configId":"same","label":"One","location":"local","endpointUrl":"http://127.0.0.1/v1","model":"m","timeoutMs":1000,"authenticated":false},{"configId":"same","label":"Two","location":"local","endpointUrl":"http://127.0.0.1/v1","model":"m","timeoutMs":1000,"authenticated":false}]}"#,
        )
        .expect("write duplicate fixture");
        assert!(ModelConfigStore::new(directory.path().to_owned())
            .inspect()
            .is_err());

        fs::remove_file(&path).expect("remove duplicate fixture");
        let store = ModelConfigStore::new(directory.path().to_owned());
        for index in 0..32 {
            let mut config = local_config();
            config.config_id = format!("local-{index}");
            store.upsert(config).expect("store allowed config");
        }
        let mut overflow = local_config();
        overflow.config_id = "local-overflow".to_owned();
        assert!(store.upsert(overflow).is_err());
    }

    #[test]
    fn invalid_replacement_does_not_change_the_prior_file() {
        let directory = TestDirectory::new();
        let store = ModelConfigStore::new(directory.path().to_owned());
        store.upsert(local_config()).expect("store original config");
        let path = directory.path().join("model-runtime-v1.json");
        let original = fs::read(&path).expect("read original config file");
        let mut invalid = local_config();
        invalid.endpoint_url = "http://example.com/v1".to_owned();
        assert!(store.upsert(invalid).is_err());
        assert_eq!(fs::read(path).expect("reread config file"), original);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_config_file() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let target = directory.path().join("target.json");
        fs::write(&target, r#"{"schemaVersion":1,"configs":[]}"#).expect("write symlink target");
        symlink(&target, directory.path().join("model-runtime-v1.json"))
            .expect("create symlink fixture");
        assert!(ModelConfigStore::new(directory.path().to_owned())
            .inspect()
            .is_err());
    }
}
