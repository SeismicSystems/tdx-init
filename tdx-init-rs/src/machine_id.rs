use crate::error::{Result, TdxInitError};
use std::time::Duration;
use tokio::fs;
use tracing::info;

const GCP_METADATA_URL: &str = "http://metadata.google.internal/computeMetadata/v1/instance/id";
const AZURE_METADATA_URL: &str = "http://169.254.169.254/metadata/instance/compute/vmId?api-version=2021-02-01&format=text";
const MACHINE_ID_PATH: &str = "/etc/machine-id";
const PRODUCT_UUID_PATH: &str = "/sys/class/dmi/id/product_uuid";

/// Fetch a unique machine identifier that persists across reboots
/// Tries multiple sources in order:
/// 1. GCP instance ID
/// 2. Azure VM ID
/// 3. /etc/machine-id (systemd)
/// 4. /sys/class/dmi/id/product_uuid (hardware UUID)
pub async fn get_machine_id() -> Result<String> {
    // Try GCP metadata
    if let Ok(id) = fetch_gcp_instance_id().await {
        info!("Using GCP instance ID as machine identifier");
        return Ok(id);
    }

    // Try Azure metadata
    if let Ok(id) = fetch_azure_vm_id().await {
        info!("Using Azure VM ID as machine identifier");
        return Ok(id);
    }

    // Try /etc/machine-id
    if let Ok(id) = fs::read_to_string(MACHINE_ID_PATH).await {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            info!("Using /etc/machine-id as machine identifier");
            return Ok(trimmed.to_string());
        }
    }

    // Try hardware UUID
    if let Ok(id) = fs::read_to_string(PRODUCT_UUID_PATH).await {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            info!("Using hardware UUID as machine identifier");
            return Ok(trimmed.to_string());
        }
    }

    Err(TdxInitError::MachineIdNotFound)
}

async fn fetch_gcp_instance_id() -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;

    let response = client
        .get(GCP_METADATA_URL)
        .header("Metadata-Flavor", "Google")
        .send()
        .await?;

    if response.status().is_success() {
        let id = response.text().await?;
        Ok(id.trim().to_string())
    } else {
        Err(TdxInitError::MetadataFetchError(
            "GCP metadata request failed".to_string(),
        ))
    }
}

async fn fetch_azure_vm_id() -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;

    let response = client
        .get(AZURE_METADATA_URL)
        .header("Metadata", "true")
        .send()
        .await?;

    if response.status().is_success() {
        let id = response.text().await?;
        Ok(id.trim().to_string())
    } else {
        Err(TdxInitError::MetadataFetchError(
            "Azure metadata request failed".to_string(),
        ))
    }
}
