use keyring::{Entry, Error};

const SERVICE: &str = "com.aiknowledgesort.model-runtime";

pub(crate) trait CredentialVault: Send + Sync {
    fn read(&self, config_id: &str) -> Result<Option<String>, String>;
    fn set(&self, config_id: &str, api_key: &str) -> Result<(), String>;
    fn delete(&self, config_id: &str) -> Result<(), String>;
}

pub(crate) struct SystemCredentialVault;

impl SystemCredentialVault {
    fn entry(config_id: &str) -> Result<Entry, String> {
        Entry::new(SERVICE, config_id)
            .map_err(|error| format!("System credential vault is unavailable: {error}"))
    }
}

impl CredentialVault for SystemCredentialVault {
    fn read(&self, config_id: &str) -> Result<Option<String>, String> {
        match Self::entry(config_id)?.get_password() {
            Ok(api_key) => Ok(Some(api_key)),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("Model credential cannot be read: {error}")),
        }
    }

    fn set(&self, config_id: &str, api_key: &str) -> Result<(), String> {
        Self::entry(config_id)?
            .set_password(api_key)
            .map_err(|error| format!("Model credential cannot be stored: {error}"))
    }

    fn delete(&self, config_id: &str) -> Result<(), String> {
        match Self::entry(config_id)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("Model credential cannot be deleted: {error}")),
        }
    }
}
