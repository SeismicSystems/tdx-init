use crate::config::{InitConfig, LuksToken};
use crate::error::{Result, TdxInitError};
use crate::utils::command::{execute_command, execute_command_with_stdin};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::process::Command;
use tracing::{info, warn};

pub const HEADER_FILE: &str = "/tmp/luks_header";
pub const MAPPER_NAME: &str = "persistent";
pub const MAPPER_DEVICE: &str = "/dev/mapper/persistent";

pub async fn is_luks_device(device_path: &std::path::Path) -> Result<bool> {
    use tokio::time::{timeout, Duration};

    info!("Checking if device is LUKS encrypted (with 60s timeout)...");
    let check_timeout = Duration::from_secs(60);

    let result = timeout(
        check_timeout,
        Command::new("cryptsetup")
            .arg("isLuks")
            .arg(device_path.as_os_str())
            .output()
    ).await;

    match result {
        Ok(Ok(output)) => {
            let is_luks = output.status.success();
            info!("LUKS check completed: is_luks={}", is_luks);
            Ok(is_luks)
        }
        Ok(Err(e)) => {
            warn!("cryptsetup isLuks command failed: {}", e);
            Err(TdxInitError::CommandError {
                cmd: "cryptsetup isLuks".to_string(),
                stderr: e.to_string(),
            })
        }
        Err(_) => {
            warn!("⏱️  cryptsetup isLuks timed out after 60s - disk I/O is hanging!");
            warn!("This suggests the io_timeout issue is affecting disk operations.");
            warn!("Assuming device is NOT LUKS encrypted (will attempt to format it).");
            Ok(false)  // Assume not LUKS if we can't check
        }
    }
}

pub async fn extract_config(device_path: &std::path::Path) -> Result<InitConfig> {
    let token = extract_luks_token(device_path).await?;

    if let Some(config_data) = token.user_data.get("config") {
        let config: InitConfig = serde_json::from_str(config_data).map_err(TdxInitError::Json)?;
        return Ok(config);
    }

    if let Some(key_data) = token.user_data.get("metadata") {
        return Ok(InitConfig {
            ssh_keys: vec![key_data.clone()],
            domain: None,
            args: None,
            log: None,
        });
    }

    Err(TdxInitError::ConfigNotFound(
        "config or metadata not found in LUKS token".to_string(),
    ))
}

pub async fn extract_luks_token(device_path: &std::path::Path) -> Result<LuksToken> {
    let cmd = Command::new("cryptsetup")
        .arg("token")
        .arg("export")
        .arg("--token-id")
        .arg("1")
        .arg(device_path.as_os_str())
        .output()
        .await
        .map_err(|e| TdxInitError::CommandError {
            cmd: "cryptsetup token export".to_string(),
            stderr: e.to_string(),
        })?;
    let token: LuksToken =
        serde_json::from_slice(&cmd.stdout).map_err(|e| TdxInitError::Json(e))?;
    Ok(token)
}

pub async fn format_luks_device(device_path: &PathBuf, passphrase: &str) -> Result<()> {
    info!("━━━ Formatting disk with LUKS2 ━━━");
    info!("Device: {}", device_path.display());
    info!("Header file: {}", HEADER_FILE);
    info!("Payload alignment: 32769 sectors");
    info!("Starting cryptsetup luksFormat...");
    let start = std::time::Instant::now();

    let result = execute_command_with_stdin(
        "cryptsetup",
        &[
            "luksFormat",
            "--type",
            "luks2",
            "--header",
            HEADER_FILE,
            "--align-payload",
            "32769",
            "--use-urandom",
            "-q",
            device_path.to_str().unwrap(),
        ],
        passphrase.as_bytes(),
    )
    .await;

    let duration = start.elapsed();
    match &result {
        Ok(_) => info!(
            "✓ LUKS format completed successfully in {:.2}s",
            duration.as_secs_f64()
        ),
        Err(e) => warn!(
            "❌ LUKS format failed after {:.2}s: {}",
            duration.as_secs_f64(),
            e
        ),
    }
    result
}

pub async fn create_luks_token(
    ssh_key: &str,
    config_data: Option<String>,
    salt: &str,
) -> Result<LuksToken> {
    let mut user_data = HashMap::new();
    user_data.insert("ssh_key".to_string(), ssh_key.to_string());
    user_data.insert("metadata".to_string(), ssh_key.to_string());
    user_data.insert("salt".to_string(), salt.to_string());

    if let Some(config_data) = config_data {
        user_data.insert("config".to_string(), config_data);
        info!("Including configuration data in LUKS header");
    }

    Ok(LuksToken {
        token_type: "user".to_string(),
        keyslots: vec![],
        user_data,
    })
}

pub async fn extract_salt(device_path: &std::path::Path) -> Result<String> {
    let token = extract_luks_token(device_path).await?;
    token
        .user_data
        .get("salt")
        .cloned()
        .ok_or(TdxInitError::MissingSalt)
}

pub async fn import_luks_token(token: &LuksToken) -> Result<()> {
    let token_json = serde_json::to_string(token).map_err(TdxInitError::Json)?;

    info!("Saving searcher SSH key...");
    execute_command_with_stdin(
        "cryptsetup",
        &[
            "token",
            "import",
            "--token-id",
            "1",
            "--header",
            HEADER_FILE,
            "/dev/null",
        ],
        token_json.as_bytes(),
    )
    .await
}

pub async fn restore_header_to_device(device_path: &PathBuf) -> Result<()> {
    info!("━━━ Writing LUKS header to disk ━━━");
    info!("Source header file: {}", HEADER_FILE);
    info!("Target device: {}", device_path.display());
    info!("Starting cryptsetup luksHeaderRestore...");
    let start = std::time::Instant::now();

    let result = execute_command(
        "cryptsetup",
        &[
            "luksHeaderRestore",
            device_path.to_str().unwrap(),
            "--header-backup-file",
            HEADER_FILE,
        ],
    )
    .await;

    let duration = start.elapsed();
    match &result {
        Ok(_) => info!(
            "✓ Header restore completed in {:.2}s",
            duration.as_secs_f64()
        ),
        Err(e) => warn!(
            "❌ Header restore failed after {:.2}s: {}",
            duration.as_secs_f64(),
            e
        ),
    }
    result
}

pub async fn backup_header_from_device(device_path: &PathBuf) -> Result<()> {
    info!("Extracting LUKS header...");
    execute_command(
        "cryptsetup",
        &[
            "luksHeaderBackup",
            device_path.to_str().unwrap(),
            "--header-backup-file",
            HEADER_FILE,
        ],
    )
    .await
}

pub async fn open_luks_container(device_path: &PathBuf, passphrase: &str) -> Result<()> {
    info!("━━━ Opening LUKS container ━━━");
    info!("Device: {}", device_path.display());
    info!("Header file: {}", HEADER_FILE);
    info!("Mapper name: {}", MAPPER_NAME);
    info!("Starting cryptsetup open...");
    let start = std::time::Instant::now();

    let result = execute_command_with_stdin(
        "cryptsetup",
        &[
            "open",
            "--header",
            HEADER_FILE,
            device_path.to_str().unwrap(),
            MAPPER_NAME,
        ],
        passphrase.as_bytes(),
    )
    .await;

    let duration = start.elapsed();
    match &result {
        Ok(_) => {
            info!(
                "✓ LUKS container opened successfully in {:.2}s",
                duration.as_secs_f64()
            );
            info!("Mapped device available at: {}", MAPPER_DEVICE);
        }
        Err(e) => warn!(
            "❌ Failed to open LUKS container after {:.2}s: {}",
            duration.as_secs_f64(),
            e
        ),
    }
    result
}

pub async fn close_luks_container() -> Result<()> {
    execute_command("cryptsetup", &["close", MAPPER_NAME]).await
}

pub async fn cleanup_header_file() {
    let _ = fs::remove_file(HEADER_FILE).await;
}
