use anyhow::{Context, Result};
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

    // ovs-port
    let _ = Command::new("nmcli").args(["-t", "-f", "NAME", "c", "show", &port_name]).status();
    // Create or modify master relationship
    let _ = Command::new("nmcli").args(["c", "add", "type", "ovs-port", "con-name", &port_name, "ifname", ifname]).status();
    let _ = Command::new("nmcli").args(["c", "modify", &port_name, "connection.master", bridge, "connection.slave-type", "ovs-bridge"]).status();

    // ethernet slave
    let _ = Command::new("nmcli").args(["c", "add", "type", "ethernet", "con-name", &eth_name, "ifname", ifname]).status();
    let _ = Command::new("nmcli").args(["c", "modify", &eth_name, "connection.master", &port_name, "connection.slave-type", "ovs-port"]).status();
    let _ = Command::new("nmcli").args(["-w", "5", "c", "up", &eth_name]).status();
    Ok(())
}

pub fn remove_dynamic_port(ifname: &str) -> Result<()> {
    let port_name = format!("dyn-port-{ifname}");
    let eth_name = eth_conn_name(ifname);
    let _ = Command::new("nmcli").args(["c", "down", &eth_name]).status();
    let _ = Command::new("nmcli").args(["c", "delete", &eth_name]).status();
    let _ = Command::new("nmcli").args(["c", "delete", &port_name]).status();
    Ok(())
}
