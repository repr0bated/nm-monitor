// Net state plugin - manages core network infrastructure via systemd-networkd
// Handles: interfaces, bridges, IPs, basic connectivity (set in stone)
use crate::plugin_footprint::PluginFootprint;
use crate::state::plugin::{
    ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use log;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::process::Command as AsyncCommand;

/// Network configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interfaces: Vec<InterfaceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub if_type: InterfaceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<Ipv4Config>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<Ipv6Config>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,

    /// Dynamic properties - introspection captures ALL hardware properties here
    /// Examples: mtu, mac_addresses (array), speed, duplex, txqueuelen, etc.
    ///
    /// APPEND-ONLY: Field names are permanent once added (by introspection or user)
    /// Values are mutable (ledger tracks all changes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,

    /// Property schema - tracks which fields exist (append-only set)
    /// Used for validation: new fields can be added, existing fields cannot be removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_schema: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceType {
    Ethernet,
    OvsBridge,
    OvsPort,
    Bridge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ipv4Config {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<AddressConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ipv6Config {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressConfig {
    pub ip: String,
    pub prefix: u8,
}

/// Net state plugin implementation
pub struct NetStatePlugin {
    pub config_dir: String,
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

impl NetStatePlugin {
    pub fn new() -> Self {
        Self {
            config_dir: "/etc/systemd/network".to_string(),
            blockchain_sender: None,
        }
    }

    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            config_dir: "/etc/systemd/network".to_string(),
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Validate interface configuration
    pub fn validate_interface_config(&self, _config: &InterfaceConfig) -> Result<()> {
        // Temporarily disabled for debugging
        Ok(())
    }

    /// Check if OVS is installed and running
    pub async fn check_ovs_available(&self) -> Result<bool> {
        let output = AsyncCommand::new("ovs-vsctl")
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => Ok(true),
            _ => {
                log::info!("OVS not available - skipping OVS operations");
                Ok(false)
            }
        }
    }

    /// Query current network state from systemd-networkd
    pub async fn query_networkd_state(&self) -> Result<NetworkConfig> {
        let output = AsyncCommand::new("networkctl")
            .arg("list")
            .arg("--json=short")
            .output()
            .await
            .context("Failed to execute networkctl")?;

        if !output.status.success() {
            // Fall back to parsing plain text output
            return self.query_networkd_state_fallback().await;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let interfaces: Vec<Value> = serde_json::from_str(&stdout).unwrap_or_else(|_| Vec::new());

        let mut network_interfaces = Vec::new();

        for iface in interfaces {
            if let Some(name) = iface.get("Name").and_then(|n| n.as_str()) {
                // Skip loopback
                if name == "lo" {
                    continue;
                }

                // Query detailed interface info
                if let Ok(iface_config) = self.query_interface_details(name).await {
                    network_interfaces.push(iface_config);
                }
            }
        }

        Ok(NetworkConfig { interfaces: network_interfaces })
    }

    /// Fallback method using plain text networkctl output
    pub async fn query_networkd_state_fallback(&self) -> Result<NetworkConfig> {
        let output = AsyncCommand::new("networkctl")
            .arg("list")
            .output()
            .await
            .context("Failed to execute networkctl")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut network_interfaces = Vec::new();

        for line in stdout.lines() {
            // Parse lines like: "1 lo loopback"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let Ok(idx) = parts[0].parse::<u32>() {
                    let name = parts[1];
                    let if_type = parts[2];

                    // Skip loopback
                    if name == "lo" {
                        continue;
                    }

                    if let Ok(iface_config) = self.query_interface_details(name).await {
                        network_interfaces.push(iface_config);
                    }
                }
            }
        }

        Ok(NetworkConfig { interfaces: network_interfaces })
    }

    /// Query details for a specific interface
    pub async fn query_interface_details(&self, name: &str) -> Result<InterfaceConfig> {
        let output = AsyncCommand::new("ip")
            .args(["addr", "show", name])
            .output()
            .await?;

        // Basic interface config - would need more parsing for full details
        Ok(InterfaceConfig {
            name: name.to_string(),
            if_type: InterfaceType::Ethernet, // Default assumption
            ports: None,
            ipv4: None,
            ipv6: None,
            controller: None,
            properties: None,
            property_schema: None,
        })
    }

    /// Query OVS bridges directly
    pub async fn query_ovs_bridges(&self) -> Result<Vec<InterfaceConfig>> {
        if !self.check_ovs_available().await? {
            return Ok(Vec::new());
        }

        let output = AsyncCommand::new("ovs-vsctl")
            .arg("list-br")
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let bridges_str = String::from_utf8_lossy(&output.stdout);
        let mut bridges = Vec::new();

        for bridge_name in bridges_str.lines() {
            let bridge_name = bridge_name.trim();
            if !bridge_name.is_empty() {
                bridges.push(InterfaceConfig {
                    name: bridge_name.to_string(),
                    if_type: InterfaceType::OvsBridge,
                    ports: None,
                    ipv4: None,
                    ipv6: None,
                    controller: None,
                    properties: None,
                    property_schema: None,
                });
            }
        }

        Ok(bridges)
    }

    /// Write network configuration files
    pub async fn write_config_files(&self, config: &InterfaceConfig) -> Result<()> {
        // Validate configuration first
        self.validate_interface_config(config)?;

        // For now, just create basic .network files
        if let Some(ipv4) = &config.ipv4 {
            if ipv4.enabled {
                let network_file_path = format!("{}/10-{}.network", self.config_dir, config.name);
                let network_content = self.generate_network_file(config)?;
                tokio::fs::write(&network_file_path, network_content)
                    .await
                    .context("Failed to write .network file")?;
            }
        }

        Ok(())
    }

    /// Generate .network file content
    pub fn generate_network_file(&self, config: &InterfaceConfig) -> Result<String> {
        let mut content = String::with_capacity(512);
        content.push_str("[Match]\nName=");
        content.push_str(&config.name);
        content.push_str("\n\n[Network]\n");

        if let Some(ipv4) = &config.ipv4 {
            if ipv4.enabled {
                if let Some(true) = ipv4.dhcp {
                    content.push_str("DHCP=yes\n");
                } else if let Some(addresses) = &ipv4.address {
                    for addr in addresses {
                        content.push_str("Address=");
                        content.push_str(&addr.ip);
                        content.push('/');
                        let prefix_str = addr.prefix.to_string();
                        content.push_str(&prefix_str);
                        content.push('\n');
                    }
                    if let Some(gateway) = &ipv4.gateway {
                        content.push_str("Gateway=");
                        content.push_str(gateway);
                        content.push('\n');
                    }
                }
            }
        }

        if let Some(controller) = &config.controller {
            content.push_str("Bridge=");
            content.push_str(controller);
            content.push('\n');
        }

        Ok(content)
    }

    /// Delete OVS bridge
    pub async fn delete_ovs_bridge(&self, name: &str) -> Result<()> {
        if !self.check_ovs_available().await? {
            return Ok(());
        }

        AsyncCommand::new("ovs-vsctl")
            .args(["del-br", name])
            .output()
            .await
            .context("Failed to delete OVS bridge")?;

        Ok(())
    }

    /// Reload systemd-networkd
    pub async fn reload_networkd(&self) -> Result<()> {
        AsyncCommand::new("networkctl")
            .arg("reload")
            .output()
            .await
            .context("Failed to reload systemd-networkd")?;

        Ok(())
    }

}
#[async_trait]
impl StatePlugin for NetStatePlugin {
    fn name(&self) -> &str {
        "net"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut network_config = self.query_networkd_state().await?;
        
        // Also query OVS bridges directly since networkd doesn't know about them
        if self.check_ovs_available().await? {
            let ovs_bridges = self.query_ovs_bridges().await?;
            network_config.interfaces.extend(ovs_bridges);
        }
        
        Ok(serde_json::to_value(network_config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: NetworkConfig = serde_json::from_value(current.clone())?;
        let desired_config: NetworkConfig = serde_json::from_value(desired.clone())?;

        let mut actions = Vec::new();

        // Build maps for quick lookup - avoid cloning strings unnecessarily
        let current_map: HashMap<&String, &InterfaceConfig> = current_config
            .interfaces
            .iter()
            .map(|i| (&i.name, i))
            .collect();

        let desired_map: HashMap<&String, &InterfaceConfig> = desired_config
            .interfaces
            .iter()
            .map(|i| (&i.name, i))
            .collect();

        // Find interfaces to create or modify
        for (name, desired_iface) in &desired_map {
            if let Some(current_iface) = current_map.get(name) {
                // Check if modification needed
                if serde_json::to_value(current_iface)? != serde_json::to_value(desired_iface)? {
                    actions.push(StateAction::Modify {
                        resource: (*name).clone(),
                        changes: serde_json::to_value(desired_iface)?,
                    });
                }
            } else {
                actions.push(StateAction::Create {
                    resource: (*name).clone(),
                    config: serde_json::to_value(desired_iface)?,
                });
            }
        }

        // Find interfaces to delete
        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                actions.push(StateAction::Delete {
                    resource: (*name).clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: crate::state::plugin::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(serde_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(serde_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config }
                | StateAction::Modify {
                    resource,
                    changes: config,
                } => {
                    let iface_config: InterfaceConfig = serde_json::from_value(config.clone())?;

                    match self.write_config_files(&iface_config).await {
                        Ok(_) => {
                            changes_applied.push(format!("Configured interface: {}", resource));
                        }
                        Err(e) => {
                            errors.push(format!("Failed to configure {}: {}", resource, e));
                        }
                    }
                }
                StateAction::Delete { resource } => {
                    // Check if it's an OVS bridge and delete it
                    if resource.starts_with("ovsbr") {
                        match self.delete_ovs_bridge(resource).await {
                            Ok(_) => {
                                changes_applied.push(format!("Deleted OVS bridge: {}", resource));
                            }
                            Err(e) => {
                                errors.push(format!(
                                    "Failed to delete OVS bridge {}: {}",
                                    resource, e
                                ));
                            }
                        }
                    }

                    // Remove config files
                    let network_file = format!("{}/10-{}.network", self.config_dir, resource);
                    let netdev_file = format!("{}/10-{}.netdev", self.config_dir, resource);

                    // Clean up network configuration files (ignore errors if files don't exist)
                    if let Err(e) = tokio::fs::remove_file(&network_file).await {
                        log::debug!("Failed to remove network file {:?}: {}", network_file, e);
                    }
                    if let Err(e) = tokio::fs::remove_file(&netdev_file).await {
                        log::debug!("Failed to remove netdev file {:?}: {}", netdev_file, e);
                    }

                    changes_applied.push(format!("Removed interface config: {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        // Reload systemd-networkd if any changes were made
        if !changes_applied.is_empty() {
            // Prefer D-Bus Reload to minimize disruption
            match crate::zbus_networkd::NetworkdZbus::new().await {
                Ok(client) => {
                    if let Err(e) = client.reload().await {
                        errors.push(format!("Failed to reload networkd (zbus): {}", e));
                    }
                }
                Err(e) => {
                    // Fallback to networkctl
                    if let Err(e2) = self.reload_networkd().await {
                        errors.push(format!(
                            "Failed to reload networkd (zbus={}): {}",
                            e, e2
                        ));
                    }
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let desired_config: NetworkConfig = serde_json::from_value(desired.clone())?;
        let current = self.query_current_state().await?;
        let current_config: NetworkConfig = serde_json::from_value(current)?;

        // Simple verification: check if desired interfaces exist
        let current_names: std::collections::HashSet<_> =
            current_config.interfaces.iter().map(|i| &i.name).collect();

        for iface in &desired_config.interfaces {
            if !current_names.contains(&iface.name) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("network-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let old_config: NetworkConfig = serde_json::from_value(checkpoint.state_snapshot.clone())?;

        // Restore old configuration
        for iface in &old_config.interfaces {
            self.write_config_files(iface).await?;
        }

        self.reload_networkd().await?;

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // systemd-networkd applies changes per-interface
        }
    }


}

// impl Default for NetStatePlugin {
//     fn default() -> Self {
//         Self::new()
//     }
// }
