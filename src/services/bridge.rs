//! Bridge operation service for systemd-networkd OVS bridge management

use crate::networkd_dbus::NetworkdClient;
use crate::fuse;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Service for systemd-networkd OVS bridge operations
#[derive(Debug, Clone)]
pub struct BridgeService {
    bridge_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeTopology {
    pub networkd_links: Vec<crate::networkd_dbus::LinkInfo>,
    pub bridge_state: crate::networkd_dbus::BridgeState,
    pub network_state: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeValidation {
    pub bridge_exists: bool,
    pub bridge_operational: bool,
    pub networkd_responsive: bool,
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

    /// Get bridge topology via D-Bus introspection
    pub async fn get_topology(&self) -> Result<BridgeTopology> {
        debug!("Getting topology for bridge '{}' via D-Bus", self.bridge_name);

        let client = NetworkdClient::new().await?;
        
        // Get all network links
        let networkd_links = client.list_links().await
            .context("Failed to get networkd links")?;

        // Get bridge-specific state
        let bridge_state = client.get_bridge_state(&self.bridge_name).await
            .unwrap_or_else(|_| crate::networkd_dbus::BridgeState {
                exists: false,
                operational: false,
                ports: Vec::new(),
                addresses: Vec::new(),
            });

        // Get comprehensive network state
        let network_state = client.get_network_state().await
            .context("Failed to get network state")?;

        Ok(BridgeTopology {
            networkd_links,
            bridge_state,
            network_state,
        })
    }

    /// Validate bridge connectivity via D-Bus
    pub async fn validate_connectivity(&self) -> Result<BridgeValidation> {
        info!("Validating connectivity for bridge '{}' via D-Bus", self.bridge_name);

        let client = NetworkdClient::new().await?;

        // Check if bridge exists
        let bridge_exists = client.bridge_exists(&self.bridge_name).await
            .context("Failed to check bridge existence")?;

        // Get bridge operational state
        let bridge_state = if bridge_exists {
            client.get_bridge_state(&self.bridge_name).await.ok()
        } else {
            None
        };

        let bridge_operational = bridge_state
            .as_ref()
            .map(|s| s.operational)
            .unwrap_or(false);

        // Check networkd responsiveness
        let networkd_responsive = client.list_links().await.is_ok();

        // Validate bridge synchronization with FUSE bindings
        let bridge_synchronization = fuse::validate_bridge_synchronization(&self.bridge_name)
            .unwrap_or_else(|e| {
                warn!("Failed to validate bridge synchronization: {}", e);
                HashMap::new()
            });

        // Validate connectivity preservation via D-Bus
        let connectivity_preserved = self.validate_connectivity_preservation().await?;

        Ok(BridgeValidation {
            bridge_exists,
            bridge_operational,
            networkd_responsive,
            bridge_synchronization,
            connectivity_preserved,
        })
    }

    /// Validate connectivity preservation via D-Bus
    async fn validate_connectivity_preservation(&self) -> Result<bool> {
        debug!("Validating connectivity preservation via D-Bus");

        let mut checks = Vec::new();

        // Check DNS via systemd-resolved D-Bus
        checks.push(crate::command::check_dns("localhost").await);

        // Check networkd responsiveness
        let client = NetworkdClient::new().await?;
        checks.push(client.list_links().await.is_ok());

        // Check if we have operational links
        if let Ok(links) = client.list_links().await {
            let operational_count = links
                .iter()
                .filter(|link| link.operational_state == "routable" || link.operational_state == "carrier")
                .count();
            checks.push(operational_count > 0);
        }

        // All checks must pass
        Ok(checks.iter().all(|&check| check))
    }

    /// Perform atomic bridge operation via systemd-networkd
    pub async fn perform_atomic_operation(&self, operation: &str) -> Result<String> {
        info!(
            "Performing atomic operation '{}' on bridge '{}' via systemd-networkd",
            operation, self.bridge_name
        );

        match operation {
            "create_checkpoint" => {
                let checkpoint_path = self
                    .create_networkd_backup()
                    .await
                    .context("Failed to create networkd backup checkpoint")?;
                Ok(format!("Checkpoint created: {}", checkpoint_path))
            }
            "validate_topology" => {
                let is_valid = self
                    .validate_topology()
                    .await
                    .context("Failed to validate bridge topology")?;
                Ok(format!(
                    "Topology validation: {}",
                    if is_valid { "PASSED" } else { "FAILED" }
                ))
            }
            "reload_networkd" => {
                let client = NetworkdClient::new().await?;
                client.reload_networkd().await
                    .context("Failed to reload networkd")?;
                Ok("systemd-networkd reloaded successfully".to_string())
            }
            _ => anyhow::bail!("Unknown atomic operation: {}", operation),
        }
    }

    /// Create systemd-networkd configuration backup
    async fn create_networkd_backup(&self) -> Result<String> {
        debug!("Creating networkd backup");

        let backup_dir = format!(
            "/tmp/systemd-network-backup-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        );

        tokio::fs::create_dir_all(&backup_dir).await
            .with_context(|| format!("Failed to create backup directory '{}'", backup_dir))?;

        // Copy all .network and .netdev files
        let network_dir = std::path::Path::new("/etc/systemd/network");
        if network_dir.exists() {
            let mut entries = tokio::fs::read_dir(network_dir).await
                .context("Failed to read /etc/systemd/network")?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "network" || ext == "netdev" {
                        let file_name = path.file_name().unwrap();
                        let dest = std::path::Path::new(&backup_dir).join(file_name);
                        tokio::fs::copy(&path, &dest).await
                            .with_context(|| format!("Failed to copy {:?} to {:?}", path, dest))?;
                    }
                }
            }
        }

        info!("Created networkd backup at {}", backup_dir);
        Ok(backup_dir)
    }

    /// Validate bridge topology via D-Bus
    async fn validate_topology(&self) -> Result<bool> {
        debug!("Validating topology for bridge '{}' via D-Bus", self.bridge_name);

        let client = NetworkdClient::new().await?;

        // Check if bridge exists
        let bridge_exists = client.bridge_exists(&self.bridge_name).await?;
        if !bridge_exists {
            warn!("Bridge '{}' does not exist", self.bridge_name);
            return Ok(false);
        }

        // Check bridge state
        let bridge_state = client.get_bridge_state(&self.bridge_name).await?;
        if !bridge_state.operational {
            warn!("Bridge '{}' is not operational", self.bridge_name);
            return Ok(false);
        }

        // Check if bridge has ports
        let has_ports = !bridge_state.ports.is_empty();
        if !has_ports {
            warn!("Bridge '{}' has no ports", self.bridge_name);
        }

        Ok(has_ports)
    }
}
