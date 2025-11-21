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
    pub fn validate_and_normalize(&mut self) -> std::result::Result<(), String> {
        fn process_field(value: &mut Option<String>) -> std::result::Result<(), String> {
            if let Some(s) = value {
                let normalized = s.trim().to_lowercase();
                if normalized.is_empty() {
                    *value = None;
                } else if matches!(
                    normalized.as_str(),
                    "trace" | "debug" | "info" | "warn" | "error"
                ) {
                    *value = Some(normalized);
                } else {
                    return Err(format!(
                        "Invalid log level '{}'. Must be one of: trace, debug, info, warn, error",
                        s
                    ));
                }
            }
            Ok(())
        }

        process_field(&mut self.enclave)?;
        process_field(&mut self.reth)?;
        process_field(&mut self.summit)?;
        Ok(())
    }
}
