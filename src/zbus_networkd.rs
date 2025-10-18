use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use zbus::Connection;

/// Thin zbus wrapper around org.freedesktop.network1
pub struct NetworkdZbus {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSummary {
    pub index: u32,
    pub name: String,
    pub operational_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkDetail {
    pub index: u32,
    pub name: String,
    pub operational_state: String,
    pub administrative_state: String,
    pub address_state: String,
}

impl NetworkdZbus {
    /// Create a new connection to the system bus and networkd manager
    pub async fn new() -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;
        Ok(Self { conn })
    }

    // Async helper proxies are created inline where needed to keep API minimal

    /// Reload .network/.netdev configuration (atomic handoff trigger)
    pub async fn reload(&self) -> Result<()> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            "/org/freedesktop/network1",
            "org.freedesktop.network1.Manager",
        )
        .await?;
        proxy.call_method("Reload", &()).await?;
        Ok(())
    }

    /// List links (robust): prefers networkctl JSON to avoid signature drift.
    pub async fn list_links(&self) -> Result<Vec<LinkSummary>> {
        let out = std::process::Command::new("networkctl")
            .args(["list", "--json=short", "--no-pager"]) 
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run networkctl: {}", e))?;
        if !out.status.success() {
            return Err(anyhow::anyhow!("networkctl list failed"));
        }
        let v: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| anyhow::anyhow!("Failed to parse networkctl JSON: {}", e))?;
        let mut res = Vec::new();
        if let Some(list) = v.as_array() {
            for it in list {
                let idx = it.get("ifindex").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let name = it.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let op = it
                    .get("operational-state")
                    .and_then(|x| x.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                if !name.is_empty() {
                    res.push(LinkSummary { index: idx, name, operational_state: op });
                }
            }
        }
        Ok(res)
    }

    /// Get detailed information for a link by ifindex
    pub async fn link_detail(&self, ifindex: u32) -> Result<LinkDetail> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            format!("/org/freedesktop/network1/link/_{}", ifindex),
            "org.freedesktop.network1.Link",
        )
        .await?;

        let name: String = proxy.get_property("Name").await?;
        let operational_state: String = proxy.get_property("OperationalState").await?;
        let administrative_state: String = proxy.get_property("AdministrativeState").await?;
        let address_state: String = proxy.get_property("AddressState").await?;

        Ok(LinkDetail {
            index: ifindex,
            name,
            operational_state,
            administrative_state,
            address_state,
        })
    }

    /// Find ifindex by interface name using ListLinks
    pub async fn ifindex_by_name(&self, ifname: &str) -> Result<u32> {
        for l in self.list_links().await? {
            if l.name == ifname {
                return Ok(l.index);
            }
        }
        Err(anyhow!("Interface '{}' not managed by networkd", ifname))
    }

    /// Reconfigure a link by ifindex (Manager.ReconfigureLink)
    pub async fn reconfigure_link_index(&self, ifindex: u32) -> Result<()> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            "/org/freedesktop/network1",
            "org.freedesktop.network1.Manager",
        )
        .await?;
        proxy.call_method("ReconfigureLink", &(ifindex as i32)).await?;
        Ok(())
    }

    /// Reconfigure a link by name (looks up ifindex, then calls ReconfigureLink)
    pub async fn reconfigure_link(&self, ifname: &str) -> Result<()> {
        let idx = self.ifindex_by_name(ifname).await?;
        self.reconfigure_link_index(idx).await
    }

}
