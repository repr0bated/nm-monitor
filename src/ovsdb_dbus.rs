//! OVSDB D-Bus client
//! Connects to ovsdb-dbus-wrapper service

use anyhow::{Context, Result};
use zbus::{Connection, Proxy};

/// OVSDB D-Bus client
pub struct OvsdbClient {
    proxy: Proxy<'static>,
}

impl OvsdbClient {
    /// Connect to OVSDB D-Bus wrapper
    pub async fn new() -> Result<Self> {
        let conn = Connection::system().await
            .context("Failed to connect to system D-Bus")?;
        
        let proxy = Proxy::new(
            &conn,
            "org.openvswitch.ovsdb",
            "/org/openvswitch/ovsdb",
            "org.openvswitch.ovsdb",
        ).await.context("Failed to create OVSDB D-Bus proxy")?;

        Ok(Self { proxy })
    }

    /// Create OVS bridge via D-Bus
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        self.proxy.call("CreateBridge", &(bridge_name,)).await
            .context("Failed to create bridge via D-Bus")
    }

    /// Add port to bridge via D-Bus
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        self.proxy.call("AddPort", &(bridge_name, port_name)).await
            .context("Failed to add port via D-Bus")
    }

    /// Delete bridge via D-Bus
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        self.proxy.call("DeleteBridge", &(bridge_name,)).await
            .context("Failed to delete bridge via D-Bus")
    }

    /// Check if bridge exists via D-Bus
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        self.proxy.call("BridgeExists", &(bridge_name,)).await
            .context("Failed to check bridge existence via D-Bus")
    }

    /// List all ports on a bridge via D-Bus
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        self.proxy.call("ListBridgePorts", &(bridge_name,)).await
            .context("Failed to list bridge ports via D-Bus")
    }
}
