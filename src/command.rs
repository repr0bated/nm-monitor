//! Pure zbus operations for systemd-networkd compliance

use crate::networkd_dbus::NetworkdClient;
use anyhow::Result;
use tracing::debug;
use zbus::{Connection, Proxy};

/// Check if bridge exists via zbus
pub async fn bridge_exists(bridge_name: &str) -> bool {
    debug!("Checking bridge existence via zbus: {}", bridge_name);
    
    let client = match NetworkdClient::new().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    
    client.bridge_exists(bridge_name).await.unwrap_or(false)
}

/// Check if network interface exists via zbus
pub async fn network_interface_exists(interface: &str) -> bool {
    debug!("Checking interface existence via zbus: {}", interface);
    
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

/// Get bridge ports via zbus introspection
pub async fn get_bridge_ports(bridge_name: &str) -> Result<Vec<String>> {
    debug!("Getting bridge ports via zbus: {}", bridge_name);
    
    let client = NetworkdClient::new().await?;
    let bridge_state = client.get_bridge_state(bridge_name).await?;
    
    Ok(bridge_state.ports)
}

/// Get bridge interfaces via zbus
pub async fn get_bridge_interfaces(bridge_name: &str) -> Result<Vec<String>> {
    debug!("Getting bridge interfaces via zbus: {}", bridge_name);
    
    let client = NetworkdClient::new().await?;
    let links = client.list_links().await?;
    
    let interfaces: Vec<String> = links
        .into_iter()
        .filter(|link| link.name.contains(bridge_name) || link.name.starts_with("veth"))
        .map(|link| link.name)
        .collect();
    
    Ok(interfaces)
}

/// Check DNS resolution via zbus (systemd-resolved)
pub async fn check_dns(hostname: &str) -> bool {
    debug!("Checking DNS resolution via zbus: {}", hostname);
    
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

/// Execute command only for verification (read-only operations)
pub async fn execute_command_checked(program: &str, args: &[&str]) -> Result<bool> {
    debug!("Verification command: {} {:?}", program, args);
    
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;
    
    Ok(output.status.success())
}
    Ok(output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect())
}

/// Get OVS bridge interfaces
pub async fn get_bridge_interfaces(bridge_name: &str) -> Result<Vec<String>> {
    let output = ovs_vsctl(&["list-ifaces", bridge_name]).await?;
    Ok(output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect())
}

/// List all OVS bridges
pub async fn list_bridges() -> Result<Vec<String>> {
    let output = ovs_vsctl(&["list-br"]).await?;
    Ok(output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect())
}

/// Ping host to check connectivity
pub async fn ping_host(host: &str, count: u8, timeout: u8) -> bool {
    execute_command_checked(
        "ping",
        &["-c", &count.to_string(), "-W", &timeout.to_string(), host],
    )
    .await
    .unwrap_or(false)
}

/// Check DNS resolution
pub async fn check_dns(hostname: &str) -> bool {
    execute_command_checked("nslookup", &[hostname])
        .await
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_command_with_echo() {
        let result = execute_command("echo", &["test"]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "test");
    }

    #[tokio::test]
    async fn test_execute_command_failure() {
        let result = execute_command("false", &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bridge_exists_false() {
        let exists = bridge_exists("nonexistent-bridge-12345").await;
        assert!(!exists);
    }
}
