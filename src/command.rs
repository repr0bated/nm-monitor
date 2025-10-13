//! Command execution utilities with proper error handling and context

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::debug;

/// Execute a command and return stdout as string
pub async fn execute_command(program: &str, args: &[&str]) -> Result<String> {
    debug!("Executing command: {} {:?}", program, args);

    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to execute {}", program))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Command '{}' failed with status {}: {}",
            program,
            output.status,
            stderr
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Execute a command and return success/failure as boolean
pub async fn execute_command_checked(program: &str, args: &[&str]) -> Result<bool> {
    debug!("Executing command (checked): {} {:?}", program, args);

    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to execute {}", program))?;

    Ok(output.status.success())
}

/// Execute a command with custom error message
pub async fn execute_with_context(
    program: &str,
    args: &[&str],
    context_msg: &str,
) -> Result<String> {
    debug!(
        "Executing command: {} {:?} (context: {})",
        program, args, context_msg
    );

    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| context_msg.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}: {}", context_msg, stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Execute NetworkManager command (nmcli)
pub async fn nmcli(args: &[&str]) -> Result<String> {
    execute_with_context("nmcli", args, &format!("nmcli {:?} failed", args)).await
}

/// Execute OVS command (ovs-vsctl)
pub async fn ovs_vsctl(args: &[&str]) -> Result<String> {
    execute_with_context("ovs-vsctl", args, &format!("ovs-vsctl {:?} failed", args)).await
}

/// Execute systemd-networkd command (networkctl)
pub async fn networkctl(args: &[&str]) -> Result<String> {
    execute_with_context("networkctl", args, &format!("networkctl {:?} failed", args)).await
}

/// Check if OVS bridge exists
pub async fn bridge_exists(bridge_name: &str) -> bool {
    execute_command_checked("ovs-vsctl", &["br-exists", bridge_name])
        .await
        .unwrap_or(false)
}

/// Check if network interface exists via networkctl
pub async fn network_interface_exists(interface: &str) -> bool {
    execute_command_checked("networkctl", &["status", interface, "--no-pager"])
        .await
        .unwrap_or(false)
}

/// Get OVS bridge ports
pub async fn get_bridge_ports(bridge_name: &str) -> Result<Vec<String>> {
    let output = ovs_vsctl(&["list-ports", bridge_name]).await?;
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
