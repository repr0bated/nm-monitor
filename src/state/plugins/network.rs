// Network state plugin - manages network configuration via systemd-networkd
use crate::state::plugin::{
    ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
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

/// Network state plugin implementation
pub struct NetworkStatePlugin {
    config_dir: String,
}

impl NetworkStatePlugin {
    pub fn new() -> Self {
        Self {
            config_dir: "/etc/systemd/network".to_string(),
        }
    }

    /// Query current network state from systemd-networkd
    async fn query_networkd_state(&self) -> Result<NetworkConfig> {
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
        let interfaces: Vec<Value> = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| Vec::new());

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

        Ok(NetworkConfig {
            interfaces: network_interfaces,
        })
    }

    /// Fallback method using plain text networkctl output
    async fn query_networkd_state_fallback(&self) -> Result<NetworkConfig> {
        let output = AsyncCommand::new("networkctl")
            .arg("list")
            .output()
            .await
            .context("Failed to execute networkctl")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut interfaces = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1];
                if name != "lo" {
                    if let Ok(config) = self.query_interface_details(name).await {
                        interfaces.push(config);
                    }
                }
            }
        }

        Ok(NetworkConfig { interfaces })
    }

    /// Query details for a specific interface
    async fn query_interface_details(&self, name: &str) -> Result<InterfaceConfig> {
        let output = AsyncCommand::new("ip")
            .args(["addr", "show", name])
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Determine interface type
        let if_type = if name.starts_with("ovsbr") {
            InterfaceType::OvsBridge
        } else {
            InterfaceType::Ethernet
        };

        // Parse IP addresses
        let mut addresses = Vec::new();
        for line in stdout.lines() {
            if line.trim().starts_with("inet ") {
                if let Some(addr_part) = line.split_whitespace().nth(1) {
                    if let Some((ip, prefix_str)) = addr_part.split_once('/') {
                        if let Ok(prefix) = prefix_str.parse::<u8>() {
                            addresses.push(AddressConfig {
                                ip: ip.to_string(),
                                prefix,
                            });
                        }
                    }
                }
            }
        }

        let ipv4 = if !addresses.is_empty() {
            Some(Ipv4Config {
                enabled: true,
                dhcp: None,
                address: Some(addresses),
                gateway: None,
                dns: None,
            })
        } else {
            Some(Ipv4Config {
                enabled: false,
                dhcp: Some(true),
                address: None,
                gateway: None,
                dns: None,
            })
        };

        Ok(InterfaceConfig {
            name: name.to_string(),
            if_type,
            ports: None,
            ipv4,
            ipv6: None,
            controller: None,
        })
    }

    /// Generate .network file content
    fn generate_network_file(&self, config: &InterfaceConfig) -> String {
        let mut content = format!("[Match]\nName={}\n\n[Network]\n", config.name);

        if let Some(ipv4) = &config.ipv4 {
            if ipv4.enabled {
                if let Some(true) = ipv4.dhcp {
                    content.push_str("DHCP=yes\n");
                } else if let Some(addresses) = &ipv4.address {
                    for addr in addresses {
                        content.push_str(&format!("Address={}/{}\n", addr.ip, addr.prefix));
                    }
                    if let Some(gateway) = &ipv4.gateway {
                        content.push_str(&format!("Gateway={}\n", gateway));
                    }
                    if let Some(dns) = &ipv4.dns {
                        for dns_server in dns {
                            content.push_str(&format!("DNS={}\n", dns_server));
                        }
                    }
                }
            }
        }

        if let Some(controller) = &config.controller {
            content.push_str(&format!("Bridge={}\n", controller));
        }

        content
    }

    /// Generate .netdev file content for OVS bridges
    fn generate_netdev_file(&self, config: &InterfaceConfig) -> Option<String> {
        if config.if_type == InterfaceType::OvsBridge {
            Some(format!(
                "[NetDev]\nName={}\nKind=openvswitch\n",
                config.name
            ))
        } else {
            None
        }
    }

    /// Write network configuration files
    async fn write_config_files(&self, config: &InterfaceConfig) -> Result<()> {
        use tokio::fs;

        let network_file_path = format!("{}/10-{}.network", self.config_dir, config.name);

        // Write .network file
        let network_content = self.generate_network_file(config);
        fs::write(&network_file_path, network_content)
            .await
            .context("Failed to write .network file")?;

        // Write .netdev file if needed
        if let Some(netdev_content) = self.generate_netdev_file(config) {
            let netdev_file_path = format!("{}/10-{}.netdev", self.config_dir, config.name);
            fs::write(&netdev_file_path, netdev_content)
                .await
                .context("Failed to write .netdev file")?;
        }

        Ok(())
    }

    /// Reload systemd-networkd
    async fn reload_networkd(&self) -> Result<()> {
        let output = AsyncCommand::new("networkctl")
            .arg("reload")
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow!("Failed to reload systemd-networkd"));
        }

        Ok(())
    }
}

#[async_trait]
impl StatePlugin for NetworkStatePlugin {
    fn name(&self) -> &str {
        "network"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let network_config = self.query_networkd_state().await?;
        Ok(serde_json::to_value(network_config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: NetworkConfig = serde_json::from_value(current.clone())?;
        let desired_config: NetworkConfig = serde_json::from_value(desired.clone())?;

        let mut actions = Vec::new();

        // Build maps for quick lookup
        let current_map: HashMap<String, &InterfaceConfig> = current_config
            .interfaces
            .iter()
            .map(|i| (i.name.clone(), i))
            .collect();

        let desired_map: HashMap<String, &InterfaceConfig> = desired_config
            .interfaces
            .iter()
            .map(|i| (i.name.clone(), i))
            .collect();

        // Find interfaces to create or modify
        for (name, desired_iface) in &desired_map {
            if let Some(current_iface) = current_map.get(name) {
                // Check if modification needed
                if serde_json::to_value(current_iface)? != serde_json::to_value(desired_iface)? {
                    actions.push(StateAction::Modify {
                        resource: name.clone(),
                        changes: serde_json::to_value(desired_iface)?,
                    });
                }
            } else {
                actions.push(StateAction::Create {
                    resource: name.clone(),
                    config: serde_json::to_value(desired_iface)?,
                });
            }
        }

        // Find interfaces to delete
        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                actions.push(StateAction::Delete {
                    resource: name.clone(),
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
                StateAction::Create { resource, config } | StateAction::Modify { resource, changes: config } => {
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
                    // Remove config files
                    let network_file = format!("{}/10-{}.network", self.config_dir, resource);
                    let netdev_file = format!("{}/10-{}.netdev", self.config_dir, resource);

                    let _ = tokio::fs::remove_file(network_file).await;
                    let _ = tokio::fs::remove_file(netdev_file).await;

                    changes_applied.push(format!("Removed interface: {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        // Reload systemd-networkd if any changes were made
        if !changes_applied.is_empty() {
            if let Err(e) = self.reload_networkd().await {
                errors.push(format!("Failed to reload networkd: {}", e));
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
        let old_config: NetworkConfig =
            serde_json::from_value(checkpoint.state_snapshot.clone())?;

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

impl Default for NetworkStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}

