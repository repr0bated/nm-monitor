use anyhow::Result;
use log::info;
use std::future;
use zbus::{fdo::IntrospectableProxy, ConnectionBuilder};

// use crate::ledger::Ledger; // reserved for future action logging via DBus
use crate::nmcli_dyn;
// use std::path::PathBuf; // reserved for future file parameterization

pub struct AppState {
    pub bridge: String,
    pub ledger_path: String,
}
pub struct PortAgent {
    state: AppState,
}

impl PortAgent {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

#[zbus::dbus_interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    fn ping(&self) -> String {
        "pong".into()
    }

    fn list_ports(&self) -> zbus::fdo::Result<Vec<String>> {
        nmcli_dyn::list_connection_names()
            .map(|v| {
                v.into_iter()
                    .filter(|n| n.starts_with("ovs-eth-"))
                    .map(|n| n.trim_start_matches("ovs-eth-").to_string())
                    .collect()
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    fn add_port(&self, name: &str) -> zbus::fdo::Result<String> {
        let interfaces_path = "/etc/network/interfaces".to_string();
        let managed_tag = "ovs-port-agent".to_string();
        let enable_rename = true;
        let naming_template = "vi{container}".to_string();
        let vmid: u32 = 0;

        let bridge = self.state.bridge.clone();
        let ledger_path = self.state.ledger_path.clone();

        tokio::runtime::Handle::current()
            .block_on(async {
                crate::netlink::create_container_interface(
                    bridge,
                    name,
                    name,
                    vmid,
                    interfaces_path,
                    managed_tag,
                    enable_rename,
                    naming_template,
                    ledger_path,
                )
                .await
            })
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to create container interface: {}", e))
            })?;

        Ok(format!("Container interface created for {}", name))
    }

    fn del_port(&self, name: &str) -> zbus::fdo::Result<String> {
        let interfaces_path = "/etc/network/interfaces".to_string();
        let managed_tag = "ovs-port-agent".to_string();
        let bridge = self.state.bridge.clone();
        let ledger_path = self.state.ledger_path.clone();

        tokio::runtime::Handle::current()
            .block_on(async {
                crate::netlink::remove_container_interface(
                    bridge,
                    name,
                    interfaces_path,
                    managed_tag,
                    ledger_path,
                )
                .await
            })
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to remove container interface: {}", e))
            })?;

        Ok(format!("Container interface {} removed", name))
    }

    fn introspect_network_manager(&self) -> zbus::fdo::Result<String> {
        match tokio::runtime::Handle::current().block_on(async { introspect_nm().await }) {
            Ok(_) => Ok("NetworkManager introspection completed successfully".to_string()),
            Err(e) => Err(zbus::fdo::Error::Failed(format!(
                "NetworkManager introspection failed: {}",
                e
            ))),
        }
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
    Ok(())
}

pub async fn introspect_nm() -> Result<()> {
    info!("Performing comprehensive D-Bus introspection on NetworkManager");
    let conn = zbus::Connection::system().await?;
    introspect_object(
        &conn,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
    )
    .await?;
    Ok(())
}

async fn introspect_object(
    conn: &zbus::Connection,
    destination: &str,
    path: &str,
) -> Result<String> {
    match IntrospectableProxy::builder(conn)
        .destination(destination)?
        .path(path)?
        .build()
        .await
    {
        Ok(proxy) => match proxy.introspect().await {
            Ok(xml) => Ok(xml),
            Err(e) => Err(anyhow::anyhow!("Failed to introspect {}: {}", path, e)),
        },
        Err(e) => Err(anyhow::anyhow!(
            "Failed to create proxy for {}: {}",
            path,
            e
        )),
    }
}
