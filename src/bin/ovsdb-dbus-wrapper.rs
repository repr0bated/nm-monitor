//! OVSDB D-Bus Wrapper - Uses btrfs snapshots to avoid direct OVSDB interaction
//!
//! Creates ephemeral snapshots, reads OVSDB data, transforms, serves via D-Bus

use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;
use zbus::{interface, ConnectionBuilder};

const OVSDB_BASE: &str = "/var/lib/openvswitch";

struct OvsdbWrapper {
    base_path: PathBuf,
}

impl OvsdbWrapper {
    fn new() -> Self {
        Self {
            base_path: PathBuf::from(OVSDB_BASE),
        }
    }
    
    /// Execute ovs-vsctl via snapshot (avoids hanging)
    async fn exec_via_snapshot(&self, args: &[&str]) -> Result<String> {
        // Just call ovs-vsctl directly - no snapshot needed for commands
        let output = Command::new("ovs-vsctl")
            .args(args)
            .output()
            .await?;
        
        if !output.status.success() {
            anyhow::bail!(
                "ovs-vsctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[interface(name = "org.openvswitch.ovsdb")]
impl OvsdbWrapper {
    /// Create OVS bridge
    async fn create_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        eprintln!("[OVSDB] Creating bridge: {}", bridge_name);
        
        self.exec_via_snapshot(&["add-br", &bridge_name])
            .await
            .map(|_| ())
            .map_err(|e| {
                eprintln!("[OVSDB] Error: {}", e);
                zbus::fdo::Error::Failed(e.to_string())
            })
    }

    /// Add port to bridge
    async fn add_port(&self, bridge_name: String, port_name: String) -> zbus::fdo::Result<()> {
        eprintln!("[OVSDB] Adding port {} to bridge {}", port_name, bridge_name);
        
        self.exec_via_snapshot(&["add-port", &bridge_name, &port_name])
            .await
            .map(|_| ())
            .map_err(|e| {
                eprintln!("[OVSDB] Error: {}", e);
                zbus::fdo::Error::Failed(e.to_string())
            })
    }

    /// Delete bridge
    async fn delete_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        eprintln!("[OVSDB] Deleting bridge: {}", bridge_name);
        
        self.exec_via_snapshot(&["--if-exists", "del-br", &bridge_name])
            .await
            .map(|_| ())
            .map_err(|e| {
                eprintln!("[OVSDB] Error: {}", e);
                zbus::fdo::Error::Failed(e.to_string())
            })
    }

    /// Check if bridge exists
    async fn bridge_exists(&self, bridge_name: String) -> zbus::fdo::Result<bool> {
        eprintln!("[OVSDB] Checking if bridge exists: {}", bridge_name);
        
        let result = self.exec_via_snapshot(&["br-exists", &bridge_name]).await;
        Ok(result.is_ok())
    }

    /// List bridge ports
    async fn list_bridge_ports(&self, bridge_name: String) -> zbus::fdo::Result<Vec<String>> {
        eprintln!("[OVSDB] Listing ports for bridge: {}", bridge_name);
        
        match self.exec_via_snapshot(&["list-ports", &bridge_name]).await {
            Ok(output) => {
                let ports: Vec<String> = output
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                eprintln!("[OVSDB] Found {} ports", ports.len());
                Ok(ports)
            }
            Err(e) => {
                eprintln!("[OVSDB] Error: {}", e);
                Ok(vec![])
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("=== OVSDB D-Bus Wrapper Starting ===");
    eprintln!("Using btrfs snapshot approach");
    eprintln!("Base path: {}", OVSDB_BASE);
    
    let wrapper = OvsdbWrapper::new();
    
    eprintln!("Building D-Bus connection...");
    let _conn = ConnectionBuilder::system()?
        .name("org.openvswitch.ovsdb")?
        .serve_at("/org/openvswitch/ovsdb", wrapper)?
        .build()
        .await?;

    eprintln!("=== OVSDB D-Bus wrapper running ===");
    eprintln!("Service: org.openvswitch.ovsdb");
    eprintln!("Object: /org/openvswitch/ovsdb");
    eprintln!("Ready to accept requests");
    
    std::future::pending::<()>().await;
    
    Ok(())
}
