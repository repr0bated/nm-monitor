//! Pure zbus operations for systemd-networkd compliance

use crate::networkd_dbus::NetworkdClient;
use anyhow::Result;
use tracing::debug;
use zbus::{Connection, Proxy};

pub async fn bridge_exists(bridge_name: &str) -> bool {
    let client = match NetworkdClient::new().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    client.bridge_exists(bridge_name).await.unwrap_or(false)
}

pub async fn network_interface_exists(interface: &str) -> bool {
    let client = match NetworkdClient::new().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    let links = match client.list_links().await {
        Ok(l) => l,
        Err(_) => return false,
    };
    links.iter().any(|link| link.name == interface)
}

pub async fn get_bridge_ports(bridge_name: &str) -> Result<Vec<String>> {
    let client = NetworkdClient::new().await?;
    let bridge_state = client.get_bridge_state(bridge_name).await?;
    Ok(bridge_state.ports)
}

pub async fn get_bridge_interfaces(bridge_name: &str) -> Result<Vec<String>> {
    let client = NetworkdClient::new().await?;
    let links = client.list_links().await?;
    let interfaces: Vec<String> = links
        .into_iter()
        .filter(|link| link.name.contains(bridge_name) || link.name.starts_with("veth"))
        .map(|link| link.name)
        .collect();
    Ok(interfaces)
}

pub async fn check_dns(hostname: &str) -> bool {
    let conn = match Connection::system().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    let proxy = match Proxy::new(
        &conn,
        "org.freedesktop.resolve1",
        "/org/freedesktop/resolve1",
        "org.freedesktop.resolve1.Manager",
    ).await {
        Ok(p) => p,
        Err(_) => return false,
    };
    proxy.call_method("ResolveHostname", &(0i32, hostname, 0i32, 0u64))
        .await
        .is_ok()
}

pub async fn execute_command_checked(program: &str, args: &[&str]) -> Result<bool> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;
    Ok(output.status.success())
}
