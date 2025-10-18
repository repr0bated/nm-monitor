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

        // Parse the returned JSON/YAML to check if bridge exists
        // For now, assume it exists if no error occurred
        Ok(true)
    }

    /// List all ports via D-Bus
    pub async fn list_ports(&self) -> Result<Vec<String>> {
        self.proxy.call("ListPorts", &()).await
            .context("Failed to list ports via D-Bus")
    }
}
