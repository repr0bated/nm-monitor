use anyhow::{anyhow, Context, Result};
use std::process::Command;

pub fn list_connection_names() -> Result<Vec<String>> {
    let output = Command::new("nmcli").args(["-t", "-f", "NAME", "c", "show"]).output()
        .with_context(|| "nmcli c show")?;
    if !output.status.success() { return Ok(vec![]); }
    let names = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(names)
}

pub fn eth_conn_name(ifname: &str) -> String {
    format!("dyn-eth-{ifname}")
}

pub fn ensure_dynamic_port(bridge: &str, ifname: &str) -> Result<()> {
    let port_name = format!("dyn-port-{ifname}");
    let eth_name = eth_conn_name(ifname);

    // Helper to check if a connection exists by name
    let conn_exists = |name: &str| -> bool {
        Command::new("nmcli")
            .args(["-t", "-f", "NAME", "c", "show", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    // Ensure ovs-port connection exists and is enslaved to the bridge
    if !conn_exists(&port_name) {
        let status = Command::new("nmcli")
            .args(["c", "add", "type", "ovs-port", "con-name", &port_name, "ifname", ifname])
            .status()
            .with_context(|| format!("nmcli add ovs-port {port_name}"))?;
        if !status.success() {
            return Err(anyhow!("nmcli add ovs-port failed for {port_name}"));
        }
    }
    let status = Command::new("nmcli")
        .args([
            "c",
            "modify",
            &port_name,
            "connection.master",
            bridge,
            "connection.slave-type",
            "ovs-bridge",
        ])
        .status()
        .with_context(|| format!("nmcli modify master/slave for {port_name}"))?;
    if !status.success() {
        return Err(anyhow!("nmcli modify failed for {port_name}"));
    }

    // Ensure ethernet slave exists and is enslaved to the ovs-port, with L3 disabled per docs
    if !conn_exists(&eth_name) {
        let status = Command::new("nmcli")
            .args(["c", "add", "type", "ethernet", "con-name", &eth_name, "ifname", ifname])
            .status()
            .with_context(|| format!("nmcli add ethernet {eth_name}"))?;
        if !status.success() {
            return Err(anyhow!("nmcli add ethernet failed for {eth_name}"));
        }
    }
    // Conformance: slaves must have IP configuration disabled; master handles L3 on ovs-interface
    let status = Command::new("nmcli")
        .args([
            "c",
            "modify",
            &eth_name,
            "connection.master",
            &port_name,
            "connection.slave-type",
            "ovs-port",
        ])
        .status()
        .with_context(|| format!("nmcli modify master/slave for {eth_name}"))?;
    if !status.success() {
        return Err(anyhow!("nmcli modify failed for {eth_name}"));
    }
    let _ = Command::new("nmcli").args(["c", "modify", &eth_name, "ipv4.method", "disabled"]).status();
    let _ = Command::new("nmcli").args(["c", "modify", &eth_name, "ipv6.method", "disabled"]).status();

    // Activate the ethernet slave; if bridge is active, NM will add it without bouncing the master
    let _ = Command::new("nmcli").args(["-w", "5", "c", "up", &eth_name]).status();
    Ok(())
}

pub fn remove_dynamic_port(ifname: &str) -> Result<()> {
    let port_name = format!("dyn-port-{ifname}");
    let eth_name = eth_conn_name(ifname);
    let _ = Command::new("nmcli").args(["c", "down", &eth_name]).status();
    let _ = Command::new("nmcli").args(["c", "down", &port_name]).status();
    let _ = Command::new("nmcli").args(["c", "delete", &eth_name]).status();
    let _ = Command::new("nmcli").args(["c", "delete", &port_name]).status();
    Ok(())
}
