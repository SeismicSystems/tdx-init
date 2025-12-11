use crate::error::Result;
use crate::error::TdxInitError;

use glob::glob;
use std::path::PathBuf;
use tokio::fs;
use tokio::time::Duration;
use tracing::{info, warn};

const DISCOVER_RETRY_DELAY: Duration = Duration::from_secs(2);

const PERSISTENT_DISK_GLOB: &str = "/dev/disk/by-path/*10";
const GLOB_PATTERNS_FILE: &str = "/etc/tdx-init/disk-glob";

pub async fn discover_persistent_disk_with_retry() -> Result<PathBuf> {
    info!("Starting persistent disk discovery...");
    let mut retry_count = 0;
    loop {
        info!("Disk discovery attempt #{}", retry_count + 1);
        if let Some(device) = discover_persistent_disk().await? {
            info!("Persistent disk found at: {}", device.display());
            // Try to set io_timeout, but don't fail if it doesn't work
            // (on GCP, accessing queue/io_timeout can hang due to kernel/udev issues)
            if let Err(e) = set_max_io_timeout_with_timeout(&device).await {
                warn!("Failed to set io_timeout (will proceed anyway): {}", e);
            }
            return Ok(device);
        }
        retry_count += 1;
        info!(
            "Disk not found yet, waiting {} seconds before retry #{}...",
            DISCOVER_RETRY_DELAY.as_secs(),
            retry_count + 1
        );
        tokio::time::sleep(DISCOVER_RETRY_DELAY).await;
    }
}

pub async fn discover_persistent_disk() -> Result<Option<PathBuf>> {
    let patterns = read_glob_patterns().await;
    info!(
        "Searching for persistent disk using {} pattern(s)",
        patterns.len()
    );

    for (i, pattern) in patterns.iter().enumerate() {
        info!("Trying pattern #{}: {}", i + 1, pattern);
        if let Some(device) = try_pattern(&pattern)? {
            info!(
                "✓ Pattern '{}' matched device: {}",
                pattern,
                device.display()
            );
            return Ok(Some(device));
        }
        info!("✗ Pattern '{}' matched no devices", pattern);
    }
    info!("No devices found matching any pattern");
    Ok(None)
}

fn try_pattern(pattern: &str) -> Result<Option<PathBuf>> {
    let paths = glob(pattern).map_err(TdxInitError::GlobPatternError)?;
    let mut devices = Vec::new();
    for path_result in paths {
        let path = path_result.map_err(|e| TdxInitError::GlobError(e))?;
        devices.push(path);
    }
    match devices.len() {
        0 => Ok(None),
        1 => {
            info!("Using persistent disk device: {}", devices[0].display());
            Ok(Some(devices[0].clone()))
        }
        _ => {
            warn!(
                "Multiple devices found by pattern '{}': {:?}",
                pattern, devices
            );
            info!("Using persistent disk device: {}", devices[0].display());
            Ok(Some(devices[0].clone()))
        }
    }
}

async fn read_glob_patterns() -> Vec<String> {
    let mut patterns = vec![PERSISTENT_DISK_GLOB.to_string()];
    if let Ok(data) = std::fs::read_to_string(GLOB_PATTERNS_FILE) {
        let trimmed = data.trim();
        if !data.is_empty() {
            patterns.extend(trimmed.split('\n').map(|line| line.trim().to_string()));
        }
    }
    patterns
}

async fn set_max_io_timeout_with_timeout(device_path: &PathBuf) -> Result<()> {
    info!("━━━ Attempting to set io_timeout for {} ━━━", device_path.display());

    // Set a 5-second timeout for this operation
    // (on GCP, sysfs access can hang due to kernel/udev issues)
    let timeout_duration = Duration::from_secs(5);

    match tokio::time::timeout(timeout_duration, set_max_io_timeout(device_path)).await {
        Ok(result) => result,
        Err(_) => {
            warn!("⏱️  Timeout after 5s trying to set io_timeout - sysfs access is hanging!");
            warn!("This is likely a kernel/udev race condition on GCP.");
            warn!("Proceeding without setting io_timeout (encryption may be slower or timeout).");
            Err(TdxInitError::CommandError {
                cmd: "set io_timeout".to_string(),
                stderr: "Operation timed out after 5 seconds".to_string(),
            })
        }
    }
}

async fn set_max_io_timeout(device_path: &PathBuf) -> Result<()> {
    // Resolve symlink to get the actual block device name
    info!("Resolving device path symlink...");
    let canonical_path = std::fs::canonicalize(device_path).map_err(|e| {
        TdxInitError::CommandError {
            cmd: format!("canonicalize {}", device_path.display()),
            stderr: e.to_string(),
        }
    })?;
    info!("Canonical device path: {}", canonical_path.display());

    // Extract device name (e.g., nvme0n2 from /dev/nvme0n2)
    let device_name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| TdxInitError::CommandError {
            cmd: "extract device name".to_string(),
            stderr: format!("Invalid device path: {}", canonical_path.display()),
        })?;
    info!("Block device name: {}", device_name);

    // Construct sysfs path for io_timeout
    let timeout_path = format!("/sys/block/{}/queue/io_timeout", device_name);
    info!("Sysfs timeout path: {}", timeout_path);

    // Read current timeout value
    if let Ok(current) = fs::read_to_string(&timeout_path).await {
        info!("Current io_timeout value: {}", current.trim());
    } else {
        warn!("Could not read current io_timeout value");
    }

    info!("Writing new io_timeout value: 4294967295 (u32::MAX)");
    // Write max timeout value (u32::MAX)
    fs::write(&timeout_path, "4294967295").await.map_err(|e| {
        TdxInitError::CommandError {
            cmd: format!("write to {}", timeout_path),
            stderr: e.to_string(),
        }
    })?;

    // Verify the write succeeded
    if let Ok(new_value) = fs::read_to_string(&timeout_path).await {
        info!("✓ Verified io_timeout set to: {}", new_value.trim());
    }

    info!("━━━ io_timeout configuration complete ━━━");
    Ok(())
}
