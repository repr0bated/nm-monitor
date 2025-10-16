// Net state plugin - manages core network infrastructure via systemd-networkd
// Handles: interfaces, bridges, IPs, basic connectivity (set in stone)
use crate::plugin_footprint::PluginFootprint;
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
    config_dir: String,
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

    /// Create footprint for network operations
    fn create_footprint(&self, operation: &str, data: &Value) -> Result<()> {
        if let Some(sender) = &self.blockchain_sender {
            let mut metadata = HashMap::new();
            metadata.insert("plugin".to_string(), Value::String("network".to_string()));
            metadata.insert("host".to_string(), Value::String(
                gethostname::gethostname().to_string_lossy().to_string()
            ));

            let footprint = crate::plugin_footprint::FootprintGenerator::new("network")
                .create_footprint(operation, data, Some(metadata))?;
            
            sender.send(footprint)?;
        }
        Ok(())
    }

    /// Validate interface configuration
    fn validate_interface_config(&self, config: &InterfaceConfig) -> Result<()> {
        // Validate interface name (max 15 chars for Linux)
        if config.name.len() > 15 {
            return Err(anyhow!(
                "Interface name '{}' exceeds 15 character limit",
                config.name
            ));
        }

        // Validate interface name characters (alphanumeric, dash, underscore)
        if !config
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow!(
                "Interface name '{}' contains invalid characters",
                config.name
            ));
        }

        // Validate property schema: if schema exists, all fields must be present
        if let Some(ref schema) = config.property_schema {
            if let Some(ref properties) = config.properties {
                // Check that all schema fields exist in properties
                for field in schema {
                    if !properties.contains_key(field) {
                        return Err(anyhow!(
                            "Property '{}' declared in schema but missing from properties (append-only violation)",
                            field
                        ));
                    }
                }
            } else if !schema.is_empty() {
                return Err(anyhow!(
                    "Property schema exists but properties map is missing"
                ));
            }
        }

        // Validate OVS bridge configuration
        if config.if_type == InterfaceType::OvsBridge {
            // Validate IP configuration
            if let Some(ipv4) = &config.ipv4 {
                if ipv4.enabled && ipv4.dhcp == Some(false) {
                    // Static IP requires address
                    if ipv4.address.is_none() || ipv4.address.as_ref().unwrap().is_empty() {
                        return Err(anyhow!(
                            "Static IP enabled for {} but no address specified",
                            config.name
                        ));
                    }

                    // Validate IP addresses
                    if let Some(addresses) = &ipv4.address {
                        for addr in addresses {
                            // Basic IP validation
                            if !addr.ip.contains('.') || addr.prefix > 32 {
                                return Err(anyhow!(
                                    "Invalid IPv4 address/prefix: {}/{}",
                                    addr.ip,
                                    addr.prefix
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Validate enslaved interfaces
        if let Some(controller) = &config.controller {
            if config.if_type == InterfaceType::OvsBridge {
                return Err(anyhow!(
                    "OVS bridge '{}' cannot be enslaved to another bridge",
                    config.name
                ));
            }

            // Enslaved interfaces should not have IP configuration
            if let Some(ipv4) = &config.ipv4 {
                if ipv4.enabled {
                    log::warn!(
                        "Interface '{}' is enslaved to '{}' but has IP configuration - will be ignored",
                        config.name,
                        controller
                    );
                }
            }
        }

        Ok(())
    }

    /// Check if OVS is installed and running
    async fn check_ovs_available(&self) -> Result<bool> {
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
            properties: None,
            property_schema: None,
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
    /// Note: OVS bridges are created via ovs-vsctl, but systemd-networkd
    /// needs a .netdev file to recognize them as managed interfaces
    fn generate_netdev_file(&self, config: &InterfaceConfig) -> Option<String> {
        // Don't create .netdev files for OVS bridges - they're created by ovs-vsctl
        // systemd-networkd doesn't support Kind=openvswitch and gets confused
        if config.if_type == InterfaceType::OvsBridge {
            return None;
        }
        
        None
    }

    /// Create OVS bridge using OVSDB D-Bus
    async fn create_ovs_bridge(&self, name: &str) -> Result<()> {
        use crate::ovsdb_dbus::OvsdbClient;
        
        let client = OvsdbClient::new().await
            .context("Failed to connect to OVSDB")?;

        // Check if bridge already exists
        if client.bridge_exists(name).await? {
            log::info!("Bridge {} already exists", name);
            return Ok(());
        }

        // Create the bridge via D-Bus
        client.create_bridge(name).await
            .context("Failed to create OVS bridge via OVSDB D-Bus")?;

        log::info!("Created OVS bridge via OVSDB D-Bus: {}", name);
        Ok(())
    }

    /// Apply security settings to OVS bridge
    async fn apply_bridge_security(&self, bridge: &str) -> Result<()> {
        log::info!("Applying security settings to bridge: {}", bridge);

        // Disable STP (Spanning Tree Protocol) - prevents loops but can cause issues
        let stp_output = AsyncCommand::new("ovs-vsctl")
            .args(["set", "Bridge", bridge, "stp_enable=false"])
            .output()
            .await
            .context("Failed to disable STP")?;

        if !stp_output.status.success() {
            let stderr = String::from_utf8_lossy(&stp_output.stderr);
            log::warn!("Failed to disable STP on {}: {}", bridge, stderr);
        } else {
            log::info!("  ✓ Disabled STP on {}", bridge);
        }

        // Disable RSTP (Rapid Spanning Tree Protocol)
        let rstp_output = AsyncCommand::new("ovs-vsctl")
            .args(["set", "Bridge", bridge, "rstp_enable=false"])
            .output()
            .await
            .context("Failed to disable RSTP")?;

        if !rstp_output.status.success() {
            let stderr = String::from_utf8_lossy(&rstp_output.stderr);
            log::warn!("Failed to disable RSTP on {}: {}", bridge, stderr);
        } else {
            log::info!("  ✓ Disabled RSTP on {}", bridge);
        }

        // Enable multicast snooping (reduces broadcast storms)
        let mcast_output = AsyncCommand::new("ovs-vsctl")
            .args(["set", "Bridge", bridge, "mcast_snooping_enable=true"])
            .output()
            .await
            .context("Failed to enable multicast snooping")?;

        if !mcast_output.status.success() {
            let stderr = String::from_utf8_lossy(&mcast_output.stderr);
            log::warn!(
                "Failed to enable multicast snooping on {}: {}",
                bridge,
                stderr
            );
        } else {
            log::info!("  ✓ Enabled multicast snooping on {}", bridge);
        }

        // Set other protocols settings for security
        let other_config = AsyncCommand::new("ovs-vsctl")
            .args([
                "set",
                "Bridge",
                bridge,
                // Prevent MAC address table flooding
                "other-config:mac-table-size=2048",
                // MAC aging time (5 minutes)
                "other-config:mac-aging-time=300",
            ])
            .output()
            .await;

        if let Ok(output) = other_config {
            if output.status.success() {
                log::info!("  ✓ Applied flood protection settings on {}", bridge);
            }
        }

        // Add default flow rules for security (drop dangerous packets)
        self.add_security_flows(bridge).await?;

        Ok(())
    }

    /// Add security flow rules to bridge
    async fn add_security_flows(&self, bridge: &str) -> Result<()> {
        // Priority 100: Drop LLDP (Link Layer Discovery Protocol) packets
        // Prevents network topology exposure
        let _ = AsyncCommand::new("ovs-ofctl")
            .args([
                "add-flow",
                bridge,
                "priority=100,dl_type=0x88cc,actions=drop",
            ])
            .output()
            .await;

        // Priority 100: Drop CDP (Cisco Discovery Protocol) packets
        let _ = AsyncCommand::new("ovs-ofctl")
            .args([
                "add-flow",
                bridge,
                "priority=100,dl_dst=01:00:0c:cc:cc:cc,actions=drop",
            ])
            .output()
            .await;

        // Priority 100: Drop STP BPDUs (Bridge Protocol Data Units)
        // Prevents rogue switches from affecting topology
        let _ = AsyncCommand::new("ovs-ofctl")
            .args([
                "add-flow",
                bridge,
                "priority=100,dl_dst=01:80:c2:00:00:00/ff:ff:ff:ff:ff:f0,actions=drop",
            ])
            .output()
            .await;

        log::info!("  ✓ Added security flow rules to {}", bridge);
        Ok(())
    }

    /// Attach port to OVS bridge
    async fn attach_ovs_port(&self, bridge: &str, port: &str) -> Result<()> {
        // Check if port is already attached
        let list_output = AsyncCommand::new("ovs-vsctl")
            .args(["list-ports", bridge])
            .output()
            .await
            .context("Failed to list bridge ports")?;

        let ports = String::from_utf8_lossy(&list_output.stdout);
        if ports.lines().any(|p| p.trim() == port) {
            // Port already attached
            return Ok(());
        }

        // Attach the port
        let output = AsyncCommand::new("ovs-vsctl")
            .args(["add-port", bridge, port])
            .output()
            .await
            .context("Failed to attach port to bridge")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Failed to attach port {} to bridge {}: {}",
                port,
                bridge,
                stderr
            ));
        }

        log::info!("Attached port {} to bridge {}", port, bridge);
        Ok(())
    }

    /// Delete OVS bridge using OVSDB D-Bus
    async fn delete_ovs_bridge(&self, name: &str) -> Result<()> {
        use crate::ovsdb_dbus::OvsdbClient;
        
        let client = OvsdbClient::new().await
            .context("Failed to connect to OVSDB")?;

        client.delete_bridge(name).await
            .context("Failed to delete bridge via OVSDB D-Bus")?;

        log::info!("Deleted OVS bridge via OVSDB D-Bus: {}", name);
        Ok(())
    }

    /// Write network configuration files
    async fn write_config_files(&self, config: &InterfaceConfig) -> Result<()> {
        use tokio::fs;

        // Validate configuration first
        self.validate_interface_config(config)?;

        // If it's an OVS bridge, check OVS is available and create it
        if config.if_type == InterfaceType::OvsBridge {
            if !self.check_ovs_available().await? {
                return Err(anyhow!("OVS bridge {} requested but OVS not available", config.name));
            }
            self.create_ovs_bridge(&config.name).await?;

            // Attach ports if specified
            if let Some(ports) = &config.ports {
                for port in ports {
                    self.attach_ovs_port(&config.name, port).await?;
                }
            }
        }

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
impl StatePlugin for NetStatePlugin {
    fn name(&self) -> &str {
        "net"
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

                    let _ = tokio::fs::remove_file(network_file).await;
                    let _ = tokio::fs::remove_file(netdev_file).await;

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

impl Default for NetStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}
