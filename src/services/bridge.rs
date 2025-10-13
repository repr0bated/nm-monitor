//! Bridge operation service for OVS bridge management

use crate::command;
use crate::fuse;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Service for OVS bridge operations
#[derive(Debug, Clone)]
pub struct BridgeService {
    bridge_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeTopology {
    pub ovs_show: String,
    pub networkd_status: String,
    pub ports: Vec<String>,
    pub interfaces: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeValidation {
    pub ovs_bridge_exists: bool,
    pub networkd_exists: bool,
    pub bridge_active: bool,
    pub bridge_synchronization: HashMap<String, bool>,
    pub connectivity_preserved: bool,
}

impl BridgeService {
    /// Create a new bridge service
    pub fn new(bridge_name: impl Into<String>) -> Self {
        Self {
            bridge_name: bridge_name.into(),
        }
    }

    /// Get bridge topology information
    pub async fn get_topology(&self) -> Result<BridgeTopology> {
        debug!("Getting topology for bridge '{}'", self.bridge_name);

        // Get OVS show output
        let ovs_show = command::ovs_vsctl(&["show"])
            .await
            .context("Failed to get OVS show output")?;

        // Get networkd status
        let networkd_status = command::networkctl(&["status", &self.bridge_name, "--no-pager"])
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get networkd status for '{}': {}", self.bridge_name, e);
                String::from("Status unavailable")
            });

        // Get ports
        let ports = command::get_bridge_ports(&self.bridge_name)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get ports for '{}': {}", self.bridge_name, e);
                Vec::new()
            });

        // Get interfaces
        let interfaces = command::get_bridge_interfaces(&self.bridge_name)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get interfaces for '{}': {}", self.bridge_name, e);
                Vec::new()
            });

        Ok(BridgeTopology {
            ovs_show,
            networkd_status,
            ports,
            interfaces,
        })
    }

    /// Validate bridge connectivity and synchronization
    pub async fn validate_connectivity(&self) -> Result<BridgeValidation> {
        info!("Validating connectivity for bridge '{}'", self.bridge_name);

        // Validate OVS bridge exists
        let ovs_bridge_exists = command::bridge_exists(&self.bridge_name).await;

        // Validate networkd configuration exists
        let networkd_exists = command::network_interface_exists(&self.bridge_name).await;

        // Validate bridge is active
        let bridge_active = if networkd_exists {
            command::execute_command_checked("networkctl", &["status", &self.bridge_name, "--no-pager"])
                .await
                .unwrap_or(false)
        } else {
            false
        };

        // Validate bridge synchronization with FUSE bindings
        let bridge_synchronization = fuse::validate_bridge_synchronization(&self.bridge_name)
            .unwrap_or_else(|e| {
                warn!("Failed to validate bridge synchronization: {}", e);
                HashMap::new()
            });

        // Validate connectivity preservation
        let connectivity_preserved = self.validate_connectivity_preservation().await?;

        Ok(BridgeValidation {
            ovs_bridge_exists,
            networkd_exists,
            bridge_active,
            bridge_synchronization,
            connectivity_preserved,
        })
    }

    /// Validate that connectivity is preserved
    async fn validate_connectivity_preservation(&self) -> Result<bool> {
        debug!("Validating connectivity preservation");

        let mut checks = Vec::new();

        // Check DNS
        checks.push(command::check_dns("localhost").await);

        // Check if networkctl is responsive
        checks.push(
            command::execute_command_checked("networkctl", &["status"])
                .await
                .unwrap_or(false),
        );

        // Check if we have active connections
        if let Ok(output) = command::networkctl(&["list", "--no-pager"]).await {
            let active_count = output.lines().count();
            checks.push(active_count > 0);
        }

        // All checks must pass
        Ok(checks.iter().all(|&check| check))
    }

    /// Perform atomic bridge operation
    pub async fn perform_atomic_operation(&self, operation: &str) -> Result<String> {
        info!(
            "Performing atomic operation '{}' on bridge '{}'",
            operation, self.bridge_name
        );

        match operation {
            "create_checkpoint" => {
                let checkpoint_path = self.create_networkd_backup().await
                    .context("Failed to create networkd backup checkpoint")?;
                Ok(format!("Checkpoint created: {}", checkpoint_path))
            }
            "validate_topology" => {
                let is_valid = self.validate_topology().await
                    .context("Failed to validate bridge topology")?;
                Ok(format!(
                    "Topology validation: {}",
                    if is_valid { "PASSED" } else { "FAILED" }
                ))
            }
            "sync_with_proxmox" => {
                let result = self.synchronize_with_proxmox().await
                    .context("Failed to synchronize with Proxmox")?;
                Ok(format!("Proxmox sync: {}", result))
            }
            _ => anyhow::bail!("Unknown atomic operation: {}", operation),
        }
    }

    /// Create systemd-networkd backup
    async fn create_networkd_backup(&self) -> Result<String> {
        debug!("Creating networkd backup");

        let backup_dir = format!(
            "/tmp/systemd-network-backup-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        );

        std::fs::create_dir_all(&backup_dir)
            .with_context(|| format!("Failed to create backup directory '{}'", backup_dir))?;

        // Copy all .network and .netdev files
        let network_dir = Path::new("/etc/systemd/network");
        if network_dir.exists() {
            for entry in std::fs::read_dir(network_dir)
                .context("Failed to read /etc/systemd/network")?
            {
                let entry = entry?;
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "network" || ext == "netdev" {
                        let file_name = path.file_name().unwrap();
                        let dest = Path::new(&backup_dir).join(file_name);
                        std::fs::copy(&path, &dest).with_context(|| {
                            format!("Failed to copy {:?} to {:?}", path, dest)
                        })?;
                    }
                }
            }
        }

        info!("Created networkd backup at {}", backup_dir);
        Ok(backup_dir)
    }

    /// Validate bridge topology
    async fn validate_topology(&self) -> Result<bool> {
        debug!("Validating topology for bridge '{}'", self.bridge_name);

        // Check OVS level
        let ovs_valid = command::bridge_exists(&self.bridge_name).await;
        if !ovs_valid {
            warn!("OVS bridge '{}' does not exist", self.bridge_name);
            return Ok(false);
        }

        // Check networkd level
        let networkd_valid = command::network_interface_exists(&self.bridge_name).await;
        if !networkd_valid {
            warn!("Networkd interface '{}' does not exist", self.bridge_name);
            return Ok(false);
        }

        // Check if bridge has required ports
        let ports = command::get_bridge_ports(&self.bridge_name)
            .await
            .unwrap_or_default();
        let has_ports = !ports.is_empty();

        if !has_ports {
            warn!("Bridge '{}' has no ports", self.bridge_name);
        }

        Ok(has_ports)
    }

    /// Synchronize bridge with Proxmox
    async fn synchronize_with_proxmox(&self) -> Result<String> {
        debug!("Synchronizing bridge '{}' with Proxmox", self.bridge_name);

        let bindings = fuse::get_interface_bindings()
            .context("Failed to get interface bindings")?;

        let mut sync_count = 0;
        for (ovs_interface, binding) in bindings.iter() {
            if binding.bridge == self.bridge_name {
                // Ensure Proxmox API compatibility
                if let Err(e) = fuse::bind_veth_interface_enhanced(
                    &binding.proxmox_veth,
                    ovs_interface,
                    binding.vmid,
                    &binding.container_id,
                    &self.bridge_name,
                ) {
                    warn!("Failed to sync interface '{}': {}", ovs_interface, e);
                } else {
                    sync_count += 1;
                }
            }
        }

        info!(
            "Synchronized {} interfaces on bridge '{}' with Proxmox",
            sync_count, self.bridge_name
        );
        Ok(format!("Synchronized {} interfaces with Proxmox", sync_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_service_creation() {
        let service = BridgeService::new("ovsbr0");
        assert_eq!(service.bridge_name, "ovsbr0");
    }

    #[tokio::test]
    async fn test_validate_connectivity_preservation() {
        let service = BridgeService::new("test-br0");
        let result = service.validate_connectivity_preservation().await;
        // This should succeed even if the bridge doesn't exist
        // as it's checking system-level connectivity
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_perform_atomic_operation_unknown() {
        let service = BridgeService::new("test-br0");
        let result = service.perform_atomic_operation("unknown_operation").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown atomic operation"));
    }
}
