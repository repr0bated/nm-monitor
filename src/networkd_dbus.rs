//! Pure zbus interface to systemd-networkd for OVS bridge management

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zbus::{Connection, Proxy};

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkInfo {
    pub index: u32,
    pub name: String,
    pub operational_state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeState {
    pub exists: bool,
    pub operational: bool,
    pub ports: Vec<String>,
    pub addresses: Vec<String>,
}

pub struct NetworkdClient {
    conn: Connection,
}

impl NetworkdClient {
    pub async fn new() -> Result<Self> {
        let conn = Connection::system().await?;
        Ok(Self { conn })
    }

    /// List network links via zbus
    pub async fn list_links(&self) -> Result<Vec<LinkInfo>> {
        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            "/org/freedesktop/network1",
            "org.freedesktop.network1.Manager",
        ).await?;

        let links: Vec<(u32, String, zbus::zvariant::OwnedObjectPath)> = 
            proxy.call_method("ListLinks", &()).await?;

        let mut link_infos = Vec::new();
        for (index, name, path) in links {
            let link_proxy = Proxy::new(
                &self.conn,
                "org.freedesktop.network1",
                &path,
                "org.freedesktop.network1.Link",
            ).await?;

            let operational_state: String = link_proxy
                .get_property("OperationalState").await
                .unwrap_or_else(|_| "unknown".to_string());

            link_infos.push(LinkInfo {
                index,
                name,
                operational_state,
            });
        }

        Ok(link_infos)
    }

    /// Check bridge exists via zbus
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        let links = self.list_links().await?;
        Ok(links.iter().any(|link| link.name == bridge_name))
    }

    /// Get bridge state via zbus
    pub async fn get_bridge_state(&self, bridge_name: &str) -> Result<BridgeState> {
        let links = self.list_links().await?;
        
        let bridge_link = links.iter()
            .find(|link| link.name == bridge_name)
            .context("Bridge not found")?;

        let operational = bridge_link.operational_state == "routable" || 
                         bridge_link.operational_state == "carrier";

        let addresses = self.get_link_addresses(bridge_link.index).await?;

        Ok(BridgeState {
            exists: true,
            operational,
            ports: Vec::new(), // Would need additional introspection
            addresses,
        })
    }

    /// Get IP addresses via zbus
    async fn get_link_addresses(&self, link_index: u32) -> Result<Vec<String>> {
        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            &format!("/org/freedesktop/network1/link/{}", link_index),
            "org.freedesktop.network1.Link",
        ).await?;

        let addresses: Vec<(u8, Vec<u8>)> = proxy
            .get_property("Addresses").await
            .unwrap_or_default();

        let mut addr_strings = Vec::new();
        for (family, addr_bytes) in addresses {
            if family == 2 && addr_bytes.len() == 4 {
                let addr = format!("{}.{}.{}.{}", 
                    addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3]);
                addr_strings.push(addr);
            }
        }

        Ok(addr_strings)
    }

    /// Reload networkd via zbus
    pub async fn reload_networkd(&self) -> Result<()> {
        let proxy = Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            "/org/freedesktop/network1",
            "org.freedesktop.network1.Manager",
        ).await?;

        proxy.call_method("Reload", &()).await?;
        Ok(())
    }

    /// Get network state via zbus
    pub async fn get_network_state(&self) -> Result<HashMap<String, serde_json::Value>> {
        let links = self.list_links().await?;
        let mut state = HashMap::new();
        state.insert("links".to_string(), serde_json::to_value(&links)?);
        Ok(state)
    }
}
