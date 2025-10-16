//! OVSDB D-Bus Wrapper - Exposes ovs-vsctl via D-Bus
//! Simple wrapper that provides D-Bus interface to OVS operations

use anyhow::Result;
use zbus::{interface, ConnectionBuilder};
use tokio::process::Command;

struct OvsdbWrapper;

#[interface(name = "org.openvswitch.ovsdb")]
impl OvsdbWrapper {
    /// Create OVS bridge
    async fn create_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        let output = Command::new("ovs-vsctl")
            .args(["add-br", &bridge_name])
            .output()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(zbus::fdo::Error::Failed(format!("ovs-vsctl failed: {}", stderr)));
        }

        Ok(())
    }

    /// Add port to bridge
    async fn add_port(&self, bridge_name: String, port_name: String) -> zbus::fdo::Result<()> {
        let output = Command::new("ovs-vsctl")
            .args(["add-port", &bridge_name, &port_name])
            .output()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(zbus::fdo::Error::Failed(format!("ovs-vsctl failed: {}", stderr)));
        }

        Ok(())
    }

    /// Delete bridge
    async fn delete_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        let output = Command::new("ovs-vsctl")
            .args(["--if-exists", "del-br", &bridge_name])
            .output()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(zbus::fdo::Error::Failed(format!("ovs-vsctl failed: {}", stderr)));
        }

        Ok(())
    }

    /// Check if bridge exists
    async fn bridge_exists(&self, bridge_name: String) -> zbus::fdo::Result<bool> {
        let output = Command::new("ovs-vsctl")
            .args(["br-exists", &bridge_name])
            .output()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to execute: {}", e)))?;

        Ok(output.status.success())
    }

    /// List bridge ports
    async fn list_bridge_ports(&self, bridge_name: String) -> zbus::fdo::Result<Vec<String>> {
        let output = Command::new("ovs-vsctl")
            .args(["list-ports", &bridge_name])
            .output()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let ports: Vec<String> = stdout
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(ports)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let _conn = ConnectionBuilder::system()?
        .name("org.openvswitch.ovsdb")?
        .serve_at("/org/openvswitch/ovsdb", OvsdbWrapper)?
        .build()
        .await?;

    println!("OVSDB D-Bus wrapper running at org.openvswitch.ovsdb");
    println!("Wrapping ovs-vsctl commands via D-Bus interface");
    
    std::future::pending::<()>().await;
    
    Ok(())
}
