use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};
use zbus::{Connection, Proxy};

/// Systemd-networkd D-Bus client for querying network state
pub struct SystemdNetworkdClient {
    conn: Connection,
}

impl SystemdNetworkdClient {
    pub async fn new() -> Result<Self> {
        let conn = Connection::system().await?;
        Ok(Self { conn })
    }

    /// Get network state information
    pub async fn get_network_state(&self) -> Result<SystemdNetworkState> {
        let manager = Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            "/org/freedesktop/network1",
            "org.freedesktop.network1.Manager",
        )
        .await?;

        // Get network interfaces
        let interfaces: Vec<(u32, String, String, String)> = manager
            .call("ListLinks", &())
            .await?;

        let mut links = Vec::new();
        for (index, name, _, operational_state) in interfaces {
            links.push(NetworkLink {
                index,
                name,
                operational_state,
            });
        }

        Ok(SystemdNetworkState {
            links,
        })
    }

    /// Get detailed information about a specific link
    #[allow(dead_code)]
    pub async fn get_link_info(&self, index: u32) -> Result<LinkInfo> {
        let link_path = format!("/org/freedesktop/network1/link/_{}", index);
        let link = Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            link_path.as_str(),
            "org.freedesktop.network1.Link",
        )
        .await?;

        // Get link properties
        let name: String = link.get_property("Name").await?;
        let operational_state: String = link.get_property("OperationalState").await?;
        let admin_state: String = link.get_property("AdministrativeState").await?;
        let address_state: String = link.get_property("AddressState").await?;

        Ok(LinkInfo {
            index,
            name,
            operational_state,
            admin_state,
            address_state,
        })
    }

    /// List all network units
    #[allow(dead_code)]
    pub async fn list_network_units(&self) -> Result<Vec<String>> {
        // Use networkctl to list units since D-Bus API might not expose this directly
        let output = tokio::process::Command::new("networkctl")
            .args(["list", "--no-pager", "--no-legend"])
            .output()
            .await?;

        let units = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(units)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemdNetworkState {
    pub links: Vec<NetworkLink>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkLink {
    pub index: u32,
    pub name: String,
    pub operational_state: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct LinkInfo {
    pub index: u32,
    pub name: String,
    pub operational_state: String,
    pub admin_state: String,
    pub address_state: String,
}

/// Get comprehensive network state using both D-Bus and networkctl
pub async fn get_comprehensive_network_state() -> Result<serde_json::Value> {
    let mut state = serde_json::Map::new();

    // Try to get systemd-networkd state via D-Bus
    match SystemdNetworkdClient::new().await {
        Ok(client) => {
            match client.get_network_state().await {
                Ok(network_state) => {
                    state.insert("networkd_links".to_string(), serde_json::json!(network_state.links));
                }
                Err(e) => {
                    warn!("Failed to get networkd state via D-Bus: {}", e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to connect to systemd-networkd D-Bus: {}", e);
        }
    }

    // Get networkctl status
    if let Ok(output) = tokio::process::Command::new("networkctl")
        .args(["status", "--no-pager"])
        .output()
        .await {
        let status = String::from_utf8_lossy(&output.stdout);
        state.insert("networkctl_status".to_string(), serde_json::json!(status));
    }

    // Get OVS bridge states
    if let Ok(output) = tokio::process::Command::new("ovs-vsctl")
        .args(["show"])
        .output()
        .await {
        let ovs_show = String::from_utf8_lossy(&output.stdout);
        state.insert("ovs_bridges".to_string(), serde_json::json!(ovs_show));
    }

    Ok(serde_json::Value::Object(state))
}

/// Check if systemd-networkd is managing an interface
#[allow(dead_code)]
pub async fn is_interface_managed(ifname: &str) -> Result<bool> {
    let client = SystemdNetworkdClient::new().await?;
    let state = client.get_network_state().await?;
    
    Ok(state.links.iter().any(|link| link.name == ifname))
}

/// Reload systemd-networkd configuration
#[allow(dead_code)]
pub async fn reload_networkd() -> Result<()> {
    let output = tokio::process::Command::new("networkctl")
        .args(["reload"])
        .output()
        .await?;

    if !output.status.success() {
        warn!("Failed to reload systemd-networkd: {}", 
              String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

/// Get network interface status
#[allow(dead_code)]
pub async fn get_interface_status(ifname: &str) -> Result<serde_json::Value> {
    let output = tokio::process::Command::new("networkctl")
        .args(["status", ifname, "--no-pager"])
        .output()
        .await?;

    let status = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };

    Ok(serde_json::json!({
        "interface": ifname,
        "status": status,
        "success": output.status.success()
    }))
}

/// List all network units
pub async fn list_network_units() -> Result<Vec<String>> {
    let output = tokio::process::Command::new("networkctl")
        .args(["list", "--no-pager", "--no-legend"])
        .output()
        .await?;

    let units = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(units)
}
