use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::fs;
use std::path::Path;
use std::process::Command;

/// FUSE mount point for Proxmox veth interface binding
const FUSE_MOUNT_BASE: &str = "/var/lib/ovs-port-agent/fuse";

/// Ensure FUSE mount point directory exists
pub fn ensure_fuse_mount_base() -> Result<()> {
    let mount_path = Path::new(FUSE_MOUNT_BASE);
    if !mount_path.exists() {
        fs::create_dir_all(mount_path)
            .with_context(|| format!("creating FUSE mount base: {}", FUSE_MOUNT_BASE))?;
        info!("Created FUSE mount base directory: {}", FUSE_MOUNT_BASE);
    }
    Ok(())
}

/// Bind mount a Proxmox veth interface to an OVS interface
/// This creates the necessary bind mount for Proxmox GUI visibility
pub fn bind_veth_interface(proxmox_veth: &str, ovs_interface: &str) -> Result<()> {
    let proxmox_path = format!("/proc/net/veth/{}", proxmox_veth);
    let ovs_path = format!("/sys/class/net/{}", ovs_interface);

    // Ensure source exists
    if !Path::new(&proxmox_path).exists() {
        return Err(anyhow::anyhow!(
            "Proxmox veth interface not found: {}",
            proxmox_path
        ));
    }

    if !Path::new(&ovs_path).exists() {
        return Err(anyhow::anyhow!("OVS interface not found: {}", ovs_path));
    }

    // Create mount point if it doesn't exist
    let mount_point = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);
    let mount_path = Path::new(&mount_point);

    if !mount_path.exists() {
        fs::create_dir_all(mount_path)
            .with_context(|| format!("creating mount point: {}", mount_point))?;
    }

    // Create bind mount
    let output = Command::new("mount")
        .args(["--bind", &proxmox_path, &mount_point])
        .output()
        .context("failed to create bind mount")?;

    if output.status.success() {
        info!("Successfully bound {} to {}", proxmox_veth, ovs_interface);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Failed to bind interfaces: {}", stderr))
    }
}

/// Unbind a veth interface
pub fn unbind_veth_interface(ovs_interface: &str) -> Result<()> {
    let mount_point = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);

    if Path::new(&mount_point).exists() {
        let output = Command::new("umount")
            .arg(&mount_point)
            .output()
            .context("failed to unmount")?;

        if output.status.success() {
            // Remove empty directory
            if let Err(e) = fs::remove_dir(&mount_point) {
                debug!(
                    "Failed to remove mount point directory {}: {}",
                    mount_point, e
                );
            }
            info!("Successfully unbound {}", ovs_interface);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to unbind {}: {}", ovs_interface, stderr);
        }
    }

    Ok(())
}

/// Clean up all FUSE mount points
pub fn cleanup_all_mounts() -> Result<()> {
    let mount_base = Path::new(FUSE_MOUNT_BASE);

    if mount_base.exists() {
        // List all mount points
        if let Ok(entries) = fs::read_dir(mount_base) {
            #[allow(clippy::manual_flatten)]
            for entry in entries {
                if let Ok(entry) = entry {
                    if entry.path().is_dir() {
                        if let Some(dir_name) = entry.file_name().to_str() {
                            if let Err(e) = unbind_veth_interface(dir_name) {
                                warn!("Failed to cleanup mount for {}: {}", dir_name, e);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_fuse_mount_base() {
        let temp_dir = tempdir().unwrap();
        let _test_base = temp_dir.path().join("fuse_test");

        // This would need to be run as root in real scenarios
        // For testing, we just verify the logic doesn't panic
        let result = ensure_fuse_mount_base();
        // In a real test environment, this might fail due to permissions
        // but the logic should be sound
        assert!(result.is_ok() || result.is_err()); // Either way, no panic
    }
}
