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
    let conn = zbus::Connection::system().await?;
    let proxy = IntrospectableProxy::builder(&conn)
        .destination("org.freedesktop.NetworkManager")?
        .path("/org/freedesktop/NetworkManager")?
        .build()
        .await?;
    let xml = proxy.introspect().await?;
    println!("{}", xml);
    Ok(())
}
