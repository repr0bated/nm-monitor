//! OVSDB D-Bus client
//! Connects to ovsdb-dbus-wrapper service

use anyhow::{Context, Result};
use zbus::{Connection, Proxy};

/// OVSDB D-Bus client
pub struct OvsdbClient {
    proxy: Proxy<'static>,
}

impl OvsdbClient {
    /// Connect to OVSDB D-Bus wrapper (Go Open vSwitch module)
    pub async fn new() -> Result<Self> {
        let conn = Connection::system().await
            .context("Failed to connect to system D-Bus")?;

        let proxy = Proxy::new(
            &conn,
            "dev.ovs.PortAgent1",
            "/dev/ovs/PortAgent1",
            "dev.ovs.PortAgent1",
        ).await.context("Failed to create OVSDB D-Bus proxy")?;

        Ok(Self { proxy })
    }

    /// Create OVS bridge via D-Bus using ApplyState
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        let state_yaml = format!(r#"
network:
  interfaces:
    - name: {}
      type: ovs-bridge
      ports: []
      ipv4:
        enabled: false
"#, bridge_name);

        self.proxy.call("ApplyState", &(&state_yaml,)).await
            .context("Failed to create bridge via D-Bus ApplyState")
    }

    /// Add port to bridge via D-Bus using ApplyState
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        let state_yaml = format!(r#"
network:
  interfaces:
    - name: {}
      type: ovs-bridge
      ports:
        - {}
      ipv4:
        enabled: false
"#, bridge_name, port_name);

        self.proxy.call("ApplyState", &(&state_yaml,)).await
            .context("Failed to add port via D-Bus ApplyState")
    }

    /// Delete bridge via D-Bus using ApplyState with empty config
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        let state_yaml = format!(r#"
network:
  interfaces:
    - name: {}
      type: deleted
"#, bridge_name);

        self.proxy.call("ApplyState", &(&state_yaml,)).await
            .context("Failed to delete bridge via D-Bus ApplyState")
    }

    /// Check if bridge exists via D-Bus using QueryState
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        let result: String = self.proxy.call("QueryState", &("net",)).await
            .context("Failed to query bridge existence via D-Bus")?;

        // Parse the returned JSON to check if bridge exists
        let state: serde_json::Value = serde_json::from_str(&result)
            .context("Failed to parse network state JSON")?;

        // Check if the network state contains interfaces
        if let Some(network) = state.get("network") {
            if let Some(interfaces) = network.get("interfaces") {
                if let Some(interfaces_array) = interfaces.as_array() {
                    // Look for the bridge by name
                    for interface in interfaces_array {
                        if let Some(name) = interface.get("name") {
                            if let Some(name_str) = name.as_str() {
                                if name_str == bridge_name {
                                    // Check if it's actually an OVS bridge
                                    if let Some(if_type) = interface.get("type") {
                                        if let Some(type_str) = if_type.as_str() {
                                            return Ok(type_str == "ovs-bridge");
                                        }
                                    }
                                    return Ok(true); // Found interface with matching name
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(false) // Bridge not found
    }

    /// List all ports via D-Bus
    pub async fn list_ports(&self) -> Result<Vec<String>> {
        self.proxy.call("ListPorts", &()).await
            .context("Failed to list ports via D-Bus")
    }
}
