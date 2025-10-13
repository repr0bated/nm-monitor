use anyhow::{Context, Result};
use log::{info, warn};
use std::fs;
use std::path::Path;

/// Configure unmanaged devices for systemd-networkd
/// Unlike NetworkManager, systemd-networkd manages interfaces that have .network files
/// So "unmanaged" means ensuring no .network files exist for these interfaces
pub fn write_unmanaged_devices(unmanaged: &[String]) -> Result<()> {
    if unmanaged.is_empty() {
        return Ok(());
    }

    info!(
        "Configuring {} interfaces as unmanaged by systemd-networkd",
        unmanaged.len()
    );

    let network_dir = Path::new("/etc/systemd/network");

    // Ensure network directory exists
    if !network_dir.exists() {
        fs::create_dir_all(network_dir).context("creating systemd network directory")?;
    }

    // Remove any existing .network files for unmanaged interfaces
    for interface in unmanaged {
        let network_file = network_dir.join(format!("{}.network", interface));
        if network_file.exists() {
            fs::remove_file(&network_file).with_context(|| {
                format!(
                    "removing .network file for unmanaged interface {}",
                    interface
                )
            })?;
            info!(
                "Removed .network file for unmanaged interface: {}",
                interface
            );
        }

        // Also remove any .netdev files that might exist
        let netdev_file = network_dir.join(format!("{}.netdev", interface));
        if netdev_file.exists() {
            fs::remove_file(&netdev_file).with_context(|| {
                format!(
                    "removing .netdev file for unmanaged interface {}",
                    interface
                )
            })?;
            info!(
                "Removed .netdev file for unmanaged interface: {}",
                interface
            );
        }
    }

    // Reload systemd-networkd to pick up the changes
    let output = std::process::Command::new("networkctl")
        .args(["reload"])
        .output()
        .context("Failed to reload systemd-networkd")?;

    if !output.status.success() {
        warn!(
            "Failed to reload systemd-networkd after configuring unmanaged devices: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
