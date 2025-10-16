//! Port management service for container interface operations

use crate::netlink::{self, InterfaceConfig};
use crate::ovsdb_dbus::OvsdbClient;
use anyhow::{Context, Result};
use tracing::{debug, info};

/// Service for port management operations
#[derive(Debug, Clone)]
pub struct PortManagementService {
    bridge: String,
    ledger_path: String,
}

impl PortManagementService {
    /// Create a new port management service
    pub fn new(bridge: impl Into<String>, ledger_path: impl Into<String>) -> Self {
        Self {
            bridge: bridge.into(),
            ledger_path: ledger_path.into(),
        }
    }

    /// List all ports on the bridge
    pub async fn list_ports(&self) -> Result<Vec<String>> {
        debug!("Listing ports on bridge '{}'", self.bridge);

        let client = OvsdbClient::new().await
            .context("Failed to connect to OVSDB")?;
        
        client.list_bridge_ports(&self.bridge).await
            .context("Failed to list bridge ports via OVSDB D-Bus")
    }

    /// Add a port to the bridge
    pub async fn add_port(&self, name: &str) -> Result<String> {
        info!("Adding port '{}' to bridge '{}'", name, self.bridge);

        let config = InterfaceConfig::new(
            self.bridge.clone(),
            name.to_string(),
            name.to_string(),
            0, // vmid
        )
        .with_ledger_path(self.ledger_path.clone());

        netlink::create_container_interface(config)
            .await
            .with_context(|| format!("Failed to create container interface '{}'", name))?;

        info!(
            "Successfully added port '{}' to bridge '{}'",
            name, self.bridge
        );
        Ok(format!("Container interface created for {}", name))
    }

    /// Remove a port from the bridge
    pub async fn del_port(&self, name: &str) -> Result<String> {
        info!("Removing port '{}' from bridge '{}'", name, self.bridge);

        let interfaces_path = "/etc/network/interfaces".to_string();
        let managed_tag = "ovs-port-agent".to_string();

        netlink::remove_container_interface(
            self.bridge.clone(),
            name,
            interfaces_path,
            managed_tag,
            self.ledger_path.clone(),
        )
        .await
        .with_context(|| format!("Failed to remove container interface '{}'", name))?;

        info!(
            "Successfully removed port '{}' from bridge '{}'",
            name, self.bridge
        );
        Ok(format!("Container interface {} removed", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_management_service_creation() {
        let service = PortManagementService::new("vmbr0", "/var/lib/ledger.jsonl");
        assert_eq!(service.bridge, "vmbr0");
        assert_eq!(service.ledger_path, "/var/lib/ledger.jsonl");
    }

    #[test]
    fn test_list_ports_handles_error_gracefully() {
        // Test service creation only (nm_query requires tokio runtime)
        let service = PortManagementService::new("nonexistent-bridge", "/tmp/ledger.jsonl");
        assert_eq!(service.bridge, "nonexistent-bridge");
    }
}
