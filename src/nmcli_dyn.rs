use anyhow::{bail, Context, Result};
use log::{debug, info, warn};
use std::process::Command;

pub fn list_connection_names() -> Result<Vec<String>> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "c", "show"])
        .output()
        .with_context(|| "nmcli c show")?;
    if !output.status.success() {
        return Ok(vec![]);
    }
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

    info!(
        "Ensuring dynamic OVS port {} on bridge {} for interface {}",
        port_name, bridge, ifname
    );

    // Check if port already exists and is active
    let port_active = is_connection_active(&port_name)?;

    if port_active {
        debug!(
            "OVS port {} already exists and is active, skipping",
            port_name
        );
        return Ok(());
    }

    // Only delete if exists but is not active
    if connection_exists(&port_name)? {
        debug!("Deleting inactive OVS port {} for recreation", port_name);
        let _ = Command::new("nmcli")
            .args(["connection", "delete", &port_name])
            .output();
    }

    // Create OVS port using correct NM syntax (controller, not master)
    debug!("Creating OVS port {} on bridge {}", port_name, bridge);

    let output = Command::new("nmcli")
        .args([
            "conn",
            "add",
            "type",
            "ovs-port",
            "con-name",
            &port_name,
            "conn.interface",
            ifname,
            "controller",
            bridge,
        ])
        .output()
        .context("Failed to create OVS port")?;

    if !output.status.success() {
        bail!(
            "Failed to create OVS port {}: {}",
            port_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Handle ethernet slave connection - always create fresh with enslavement
    if connection_exists(&eth_name)? {
        debug!(
            "Deleting existing ethernet connection {} for recreation",
            eth_name
        );
        let _ = Command::new("nmcli")
            .args(["connection", "delete", &eth_name])
            .output();
    }

    debug!(
        "Creating ethernet connection {} for port {}",
        eth_name, port_name
    );

    // Create ethernet connection using correct NM syntax with port-type
    let output = Command::new("nmcli")
        .args([
            "conn",
            "add",
            "type",
            "ethernet",
            "con-name",
            &eth_name,
            "conn.interface",
            ifname,
            "controller",
            &port_name,
            "port-type",
            "ovs-port",
        ])
        .output()
        .context("Failed to create ethernet connection")?;

    if !output.status.success() {
        bail!(
            "Failed to create ethernet slave {}: {}",
            eth_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Activate the ethernet connection (NetworkManager will handle the port activation atomically)
    debug!("Activating ethernet connection {}", eth_name);

    let output = Command::new("nmcli")
        .args(["-w", "10", "connection", "up", &eth_name])
        .output()
        .context("Failed to activate ethernet connection")?;

    if !output.status.success() {
        warn!(
            "Failed to activate ethernet connection {}: {}",
            eth_name,
            String::from_utf8_lossy(&output.stderr)
        );
        // Not fatal - connection might already be active
    }

    info!(
        "Successfully ensured dynamic port {} on bridge {}",
        ifname, bridge
    );
    Ok(())
}

fn connection_exists(name: &str) -> Result<bool> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show", name])
        .output()
        .context("Failed to check connection existence")?;

    Ok(output.status.success())
}

fn is_connection_active(name: &str) -> Result<bool> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME,STATE", "connection", "show", "--active"])
        .output()
        .context("Failed to check active connections")?;

    if !output.status.success() {
        return Ok(false);
    }

    let active_conns = String::from_utf8_lossy(&output.stdout);
    for line in active_conns.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 2 && parts[0] == name && parts[1] == "activated" {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn remove_dynamic_port(ifname: &str) -> Result<()> {
    let port_name = format!("dyn-port-{ifname}");
    let eth_name = eth_conn_name(ifname);

    info!(
        "Removing dynamic OVS port {} for interface {}",
        port_name, ifname
    );

    // Deactivate ethernet connection first (this will deactivate the port as well)
    if connection_exists(&eth_name)? {
        debug!("Deactivating ethernet connection {}", eth_name);
        let output = Command::new("nmcli")
            .args(["connection", "down", &eth_name])
            .output()
            .context("Failed to deactivate ethernet connection")?;

        if !output.status.success() {
            warn!(
                "Failed to deactivate {}: {}",
                eth_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Delete ethernet connection
        debug!("Deleting ethernet connection {}", eth_name);
        let output = Command::new("nmcli")
            .args(["connection", "delete", &eth_name])
            .output()
            .context("Failed to delete ethernet connection")?;

        if !output.status.success() {
            warn!(
                "Failed to delete {}: {}",
                eth_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Delete OVS port connection
    if connection_exists(&port_name)? {
        debug!("Deleting OVS port {}", port_name);
        let output = Command::new("nmcli")
            .args(["connection", "delete", &port_name])
            .output()
            .context("Failed to delete OVS port")?;

        if !output.status.success() {
            warn!(
                "Failed to delete {}: {}",
                port_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    info!("Successfully removed dynamic port for interface {}", ifname);
    Ok(())
}
