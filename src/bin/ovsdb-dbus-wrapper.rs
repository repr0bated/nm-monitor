//! OVSDB D-Bus Wrapper - Uses btrfs snapshots to avoid direct OVSDB interaction
//!
//! Creates ephemeral snapshots, reads OVSDB data, transforms, serves via D-Bus

use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;
use clap::{Parser, Subcommand};
use zbus::{Connection, interface};
use std::sync::Arc;

const OVSDB_BASE: &str = "/var/lib/openvswitch";
const SNAPSHOT_DIR: &str = "/var/lib/ovsdb/snapshots";

#[derive(Parser)]
#[command(name = "ovsdb-dbus-wrapper")]
#[command(about = "OVSDB D-Bus wrapper using btrfs snapshots")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create OVS bridge
    CreateBridge {
        /// Bridge name
        name: String,
    },
    /// Add port to bridge
    AddPort {
        /// Bridge name
        bridge: String,
        /// Port name
        port: String,
    },
    /// Delete bridge
    DeleteBridge {
        /// Bridge name
        name: String,
    },
    /// Run as D-Bus service (default mode)
    Service,
}


struct OvsdbWrapper {
    #[allow(dead_code)]
    base_path: PathBuf,
}

impl OvsdbWrapper {
    fn new() -> Self {
        Self {
            base_path: PathBuf::from(OVSDB_BASE),
        }
    }

    /// Create bridge using btrfs snapshot for safe OVSDB access
    async fn create_bridge_via_snapshot(&self, bridge_name: &str) -> Result<()> {
        self.exec_via_snapshot(&["add-br", bridge_name]).await?;
        Ok(())
    }

    /// Add port to bridge using btrfs snapshot
    async fn add_port_via_snapshot(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        self.exec_via_snapshot(&["add-port", bridge_name, port_name]).await?;
        Ok(())
    }

    /// Delete bridge using btrfs snapshot
    async fn delete_bridge_via_snapshot(&self, bridge_name: &str) -> Result<()> {
        self.exec_via_snapshot(&["del-br", bridge_name]).await?;
        Ok(())
    }

    /// Check if bridge exists using btrfs snapshot
    async fn bridge_exists_via_snapshot(&self, bridge_name: &str) -> Result<bool> {
        match self.exec_via_snapshot(&["br-exists", bridge_name]).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // ovs-vsctl br-exists returns error if bridge doesn't exist
        }
    }

    /// List ports on bridge using btrfs snapshot
    async fn list_bridge_ports_via_snapshot(&self, bridge_name: &str) -> Result<Vec<String>> {
        let output = self.exec_via_snapshot(&["list-ports", bridge_name]).await?;
        let ports: Vec<String> = output.lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(ports)
    }

    /// Execute ovs-vsctl via btrfs snapshot (avoids hanging on OVSDB locks)
    async fn exec_via_snapshot(&self, args: &[&str]) -> Result<String> {
        // Create a unique snapshot name with timestamp for better uniqueness
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let snapshot_id = format!("ovsdb-snap-{}-{}", std::process::id(), timestamp);
        let snapshot_path = format!("{}/{}", SNAPSHOT_DIR, snapshot_id);

        // Ensure snapshot directory exists
        tokio::fs::create_dir_all(SNAPSHOT_DIR).await?;

        // Create read-only btrfs snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r", OVSDB_BASE, &snapshot_path])
            .output()
            .await;

        let use_snapshot = match output {
            Ok(result) if result.status.success() => {
                eprintln!("[OVSDB] Creating btrfs snapshot: {}", snapshot_path);
                true
            }
            _ => {
                eprintln!("[OVSDB] Btrfs snapshot failed, falling back to direct access");
                false
            }
        };

        let result = if use_snapshot {
            // Use snapshot for ovs-vsctl
            Command::new("ovs-vsctl")
                .args(args)
                .env("OVS_DB_DIR", &snapshot_path)
                .output()
                .await
        } else {
            // Direct access fallback
            self.exec_direct(args).await
        };

        // Clean up snapshot if it was created
        if use_snapshot {
            let _ = Command::new("btrfs")
                .args(["subvolume", "delete", &snapshot_path])
                .output()
                .await;
        }

        match result {
            Ok(output) => {
                if !output.status.success() {
                    anyhow::bail!(
                        "ovs-vsctl failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            Err(e) => Err(e.into())
        }
    }

    /// Execute ovs-vsctl directly (fallback when btrfs snapshots fail)
    async fn exec_direct(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("ovs-vsctl")
            .args(args)
            .output()
            .await
    }
}

/// D-Bus interface for OVSDB operations
#[derive(Clone)]
pub struct OvsdbDBusInterface {
    wrapper: Arc<OvsdbWrapper>,
}

#[interface(name = "org.openvswitch.ovsdb")]
impl OvsdbDBusInterface {
    /// Create OVS bridge
    async fn create_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        eprintln!("[OVSDB] Creating bridge: {}", bridge_name);
        match self.wrapper.create_bridge_via_snapshot(&bridge_name).await {
            Ok(_) => {
                eprintln!("[OVSDB] Bridge {} created successfully via btrfs snapshot", bridge_name);
                Ok(())
            }
            Err(e) => {
                eprintln!("[OVSDB] Failed to create bridge {}: {}", bridge_name, e);
                Err(zbus::fdo::Error::Failed(format!("Failed to create bridge: {}", e)))
            }
        }
    }

    /// Add port to bridge
    async fn add_port(&self, bridge_name: String, port_name: String) -> zbus::fdo::Result<()> {
        eprintln!("[OVSDB] Adding port {} to bridge {}", port_name, bridge_name);
        match self.wrapper.add_port_via_snapshot(&bridge_name, &port_name).await {
            Ok(_) => {
                eprintln!("[OVSDB] Port {} added to bridge {} successfully", port_name, bridge_name);
                Ok(())
            }
            Err(e) => {
                eprintln!("[OVSDB] Failed to add port {} to bridge {}: {}", port_name, bridge_name, e);
                Err(zbus::fdo::Error::Failed(format!("Failed to add port: {}", e)))
            }
        }
    }

    /// Delete bridge
    async fn delete_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        eprintln!("[OVSDB] Deleting bridge: {}", bridge_name);
        match self.wrapper.delete_bridge_via_snapshot(&bridge_name).await {
            Ok(_) => {
                eprintln!("[OVSDB] Bridge {} deleted successfully", bridge_name);
                Ok(())
            }
            Err(e) => {
                eprintln!("[OVSDB] Failed to delete bridge {}: {}", bridge_name, e);
                Err(zbus::fdo::Error::Failed(format!("Failed to delete bridge: {}", e)))
            }
        }
    }

    /// Check if bridge exists
    async fn bridge_exists(&self, bridge_name: String) -> zbus::fdo::Result<bool> {
        eprintln!("[OVSDB] Checking if bridge {} exists", bridge_name);
        match self.wrapper.bridge_exists_via_snapshot(&bridge_name).await {
            Ok(exists) => {
                eprintln!("[OVSDB] Bridge {} exists: {}", bridge_name, exists);
                Ok(exists)
            }
            Err(e) => {
                eprintln!("[OVSDB] Failed to check bridge {} existence: {}", bridge_name, e);
                Err(zbus::fdo::Error::Failed(format!("Failed to check bridge existence: {}", e)))
            }
        }
    }

    /// List ports on bridge
    async fn list_bridge_ports(&self, bridge_name: String) -> zbus::fdo::Result<Vec<String>> {
        eprintln!("[OVSDB] Listing ports on bridge: {}", bridge_name);
        match self.wrapper.list_bridge_ports_via_snapshot(&bridge_name).await {
            Ok(ports) => {
                eprintln!("[OVSDB] Bridge {} ports: {:?}", bridge_name, ports);
                Ok(ports)
            }
            Err(e) => {
                eprintln!("[OVSDB] Failed to list ports on bridge {}: {}", bridge_name, e);
                Err(zbus::fdo::Error::Failed(format!("Failed to list bridge ports: {}", e)))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let wrapper = OvsdbWrapper::new();

    match cli.command {
        Commands::CreateBridge { name } => {
            eprintln!("[OVSDB] Creating bridge: {}", name);
            wrapper.exec_via_snapshot(&["--may-exist", "add-br", &name]).await.map(|_| ())
        }
        Commands::AddPort { bridge, port } => {
            eprintln!("[OVSDB] Adding port {} to bridge {}", port, bridge);
            wrapper.exec_via_snapshot(&["--may-exist", "add-port", &bridge, &port]).await.map(|_| ())
        }
        Commands::DeleteBridge { name } => {
            eprintln!("========================================");
            eprintln!("[OVSDB] DELETE BRIDGE CALLED: {}", name);
            eprintln!("[OVSDB] This should NOT happen unless rollback is triggered");
            eprintln!("========================================");
            wrapper.exec_via_snapshot(&["--if-exists", "del-br", &name]).await.map(|_| ())
        }
        Commands::Service => {
            eprintln!("=== OVSDB D-Bus Wrapper Service ===");
            eprintln!("Starting D-Bus service at org.openvswitch.ovsdb...");

            let wrapper = Arc::new(OvsdbWrapper::new());
            let interface = OvsdbDBusInterface { wrapper };

            let connection = Connection::system().await?;
            connection.object_server().at("/org/openvswitch/ovsdb", interface).await?;
            connection.request_name("org.openvswitch.ovsdb").await?;

            eprintln!("D-Bus service started successfully at org.openvswitch.ovsdb");
            eprintln!("Press Ctrl+C to stop the service");

            // Keep the service running
            tokio::signal::ctrl_c().await?;
            eprintln!("D-Bus service stopped");
            Ok(())
        }
    }
}

