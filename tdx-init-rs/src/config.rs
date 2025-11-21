use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitConfig {
    pub ssh_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<DomainConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<ArgsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<LogConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclave: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuksToken {
    #[serde(rename = "type")]
    pub token_type: String,
    pub keyslots: Vec<String>,
    pub user_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclave: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summit: Option<String>,
}

impl LogConfig {
    pub fn validate_and_normalize(self) -> std::result::Result<Self, String> {
        let enclave = Self::process_field(self.enclave)?;
        let reth = Self::process_field(self.reth)?;
        let summit = Self::process_field(self.summit)?;

        Ok(LogConfig {
            enclave,
            reth,
            summit,
        })
    }

    fn process_field(value: Option<String>) -> std::result::Result<Option<String>, String> {
        match value {
            Some(s) => {
                let normalized = s.trim().to_lowercase();
                if normalized.is_empty() {
                    Ok(None)
                } else if matches!(
                    normalized.as_str(),
                    "trace" | "debug" | "info" | "warn" | "error"
                ) {
                    Ok(Some(normalized))
                } else {
                    Err(format!(
                        "Invalid log level '{}'. Must be one of: trace, debug, info, warn, error",
                        s
                    ))
                }
            }
            None => Ok(None),
        }
    }
}
