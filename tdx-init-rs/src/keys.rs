use crate::error::Result;
use crate::kdf;
use crate::luks;
use crate::passphrase;
use crate::persistence;
use crate::server;
use crate::ssh;
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;

pub async fn wait_for_key(device_path: PathBuf) -> Result<()> {
    if luks::is_luks_device(&device_path).await? {
        info!("Found existing LUKS container, extracting config and auto-mounting...");
        let config = luks::extract_config(&device_path).await?;
        ssh::write_keys(&config.ssh_keys).await?;
        persistence::write_temp_config(&config).await?;
        info!(
            "{} SSH key(s) extracted from LUKS header",
            config.ssh_keys.len()
        );

        // Extract salt and derive passphrase to auto-mount
        let salt = luks::extract_salt(&device_path).await?;
        info!("Auto-mounting disk using machine-bound key derivation...");
        passphrase::initialize_with_passphrase(device_path, &salt).await?;
        info!("Disk mounted successfully");
    } else {
        info!("No LUKS container found, starting HTTP server on port 8080...");
        let config = server::http::run_initialization_server().await?;
        ssh::write_keys(&config.ssh_keys).await?;
        persistence::write_temp_config(&config).await?;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Generate salt and derive machine-bound passphrase
        let salt = kdf::generate_salt();
        info!("Initializing disk with machine-bound encryption...");
        passphrase::initialize_with_passphrase(device_path, &salt).await?;

        info!("Configuration received via HTTP and disk initialized!");
    }
    Ok(())
}
