use anyhow::Result;
use zbus::ConnectionBuilder;
use log::info;
use std::future;

use crate::ledger::Ledger;
use crate::nmcli_dyn;
use std::path::PathBuf;

pub struct AppState {
    pub bridge: String,
    pub ledger_path: String,
}

pub struct PortAgent {
    state: AppState,
}

impl PortAgent {
    pub fn new(state: AppState) -> Self { Self { state } }
}

#[zbus::dbus_interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    /// Health check
    fn ping(&self) -> String { "pong".into() }

    /// List OVS ports on the managed bridge
    fn list_ports(&self) -> zbus::fdo::Result<Vec<String>> {
        nmcli_dyn::list_connection_names()
            .map(|v| v.into_iter().filter(|n| n.starts_with("dyn-eth-"))
                .map(|n| n.trim_start_matches("dyn-eth-").to_string()).collect())
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Add a port to the managed bridge
    fn add_port(&self, name: &str) -> zbus::fdo::Result<()> {
        nmcli_dyn::ensure_dynamic_port(&self.state.bridge, name)
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))?;
        if let Ok(mut lg) = Ledger::open(PathBuf::from(&self.state.ledger_path)) {
            let _ = lg.append("dbus_add_port", serde_json::json!({"port": name, "bridge": self.state.bridge}));
        }
        Ok(())
    }

    /// Delete a port from the managed bridge
    fn del_port(&self, name: &str) -> zbus::fdo::Result<()> {
        nmcli_dyn::remove_dynamic_port(name)
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))?;
        if let Ok(mut lg) = Ledger::open(PathBuf::from(&self.state.ledger_path)) {
            let _ = lg.append("dbus_del_port", serde_json::json!({"port": name, "bridge": self.state.bridge}));
        }
        Ok(())
    }
}

pub async fn serve_with_state(state: AppState) -> Result<()> {
    let agent = PortAgent::new(state);
    let name = "dev.ovs.PortAgent1";
    let path = "/dev/ovs/PortAgent1";
    let _conn = ConnectionBuilder::system()?
        .name(name)?
        .serve_at(path, agent)?
        .build()
        .await?;
    info!("D-Bus service registered: {} at {}", name, path);
    future::pending::<()>().await;
    // unreachable
    #[allow(unreachable_code)]
    Ok(())
}

pub async fn introspect_nm() -> Result<()> {
    use zbus::fdo::IntrospectableProxy;
    
    info!("Performing D-Bus introspection on NetworkManager");
    
    let conn = zbus::Connection::system().await?;
    
    // Introspect main NetworkManager object
    println!("=== NetworkManager Main Object ===");
    let proxy = IntrospectableProxy::builder(&conn)
        .destination("org.freedesktop.NetworkManager")?
        .path("/org/freedesktop/NetworkManager")?
        .build()
        .await?;
    let xml = proxy.introspect().await?;
    println!("{}", xml);
    
    // Introspect Settings object
    println!("\n=== NetworkManager Settings Object ===");
    let settings_proxy = IntrospectableProxy::builder(&conn)
        .destination("org.freedesktop.NetworkManager")?
        .path("/org/freedesktop/NetworkManager/Settings")?
        .build()
        .await?;
    let settings_xml = settings_proxy.introspect().await?;
    println!("{}", settings_xml);
    
    // Try to introspect OVS-specific paths if they exist
    println!("\n=== Checking for OVS-specific interfaces ===");
    
    // Get list of connections
    let list_output = std::process::Command::new("nmcli")
        .args(["-t", "-f", "UUID,TYPE", "connection", "show"])
        .output()?;
    
    if list_output.status.success() {
        let connections = String::from_utf8_lossy(&list_output.stdout);
        for line in connections.lines() {
            if let Some((uuid, conn_type)) = line.split_once(':') {
                if conn_type.contains("ovs") {
                    println!("\nFound OVS connection: {} (type: {})", uuid, conn_type);
                    
                    // Try to introspect the connection object
                    let conn_path = format!("/org/freedesktop/NetworkManager/Settings/{}", uuid.replace('-', "_"));
                    match IntrospectableProxy::builder(&conn)
                        .destination("org.freedesktop.NetworkManager")?
                        .path(&conn_path)?
                        .build()
                        .await
                    {
                        Ok(conn_proxy) => {
                            match conn_proxy.introspect().await {
                                Ok(conn_xml) => println!("Connection introspection:\n{}", conn_xml),
                                Err(e) => println!("Failed to introspect connection: {}", e),
                            }
                        }
                        Err(e) => println!("Failed to create proxy for {}: {}", conn_path, e),
                    }
                }
            }
        }
    }
    
    Ok(())
}
