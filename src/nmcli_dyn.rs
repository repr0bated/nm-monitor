use anyhow::{Context, Result, bail};
use std::process::Command;
use log::{info, debug, warn};

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

    info!("Ensuring dynamic OVS port {} on bridge {} for interface {}", port_name, bridge, ifname);

    // Check if port already exists
    let exists = connection_exists(&port_name)?;
    
    if exists {
        // Delete and recreate to ensure clean state
        debug!("Deleting existing OVS port {} for recreation", port_name);
        let _ = Command::new("nmcli")
            .args(["connection", "delete", &port_name])
            .output();
    }
    
    // Create OVS port with master relationship defined from the start
    debug!("Creating OVS port {} enslaved to bridge {}", port_name, bridge);
    
    let output = Command::new("nmcli")
        .args([
            "connection", "add",
            "type", "ovs-port",
            "con-name", &port_name,
            "ifname", ifname,
            "connection.master", bridge,
            "connection.slave-type", "ovs-bridge",
            "connection.autoconnect", "yes",
            "connection.autoconnect-priority", "50",
        ])
        .output()
        .context("Failed to create OVS port")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS port {}: {}", port_name, String::from_utf8_lossy(&output.stderr));
    }

    // Handle ethernet slave connection - always create fresh with enslavement
    if connection_exists(&eth_name)? {
        debug!("Deleting existing ethernet connection {} for recreation", eth_name);
        let _ = Command::new("nmcli")
            .args(["connection", "delete", &eth_name])
            .output();
    }
    
    debug!("Creating ethernet slave {} enslaved to port {}", eth_name, port_name);
    
    // Create ethernet connection with master/slave defined from the start
    let output = Command::new("nmcli")
        .args([
            "connection", "add",
            "type", "ethernet",
            "con-name", &eth_name,
            "ifname", ifname,
            "connection.master", &port_name,
            "connection.slave-type", "ovs-port",
            "connection.autoconnect", "yes",
            "connection.autoconnect-priority", "45",
            "802-3-ethernet.auto-negotiate", "yes",
        ])
        .output()
        .context("Failed to create ethernet slave")?;
    
    if !output.status.success() {
        bail!("Failed to create ethernet slave {}: {}", eth_name, String::from_utf8_lossy(&output.stderr));
    }

    // Activate the ethernet connection (NetworkManager will handle the port activation atomically)
    debug!("Activating ethernet connection {}", eth_name);
    
    let output = Command::new("nmcli")
        .args(["-w", "10", "connection", "up", &eth_name])
        .output()
        .context("Failed to activate ethernet connection")?;
    
    if !output.status.success() {
        warn!("Failed to activate ethernet connection {}: {}", eth_name, String::from_utf8_lossy(&output.stderr));
        // Not fatal - connection might already be active
    }
    
    info!("Successfully ensured dynamic port {} on bridge {}", ifname, bridge);
    Ok(())
}

fn connection_exists(name: &str) -> Result<bool> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show", name])
        .output()
        .context("Failed to check connection existence")?;
    
    Ok(output.status.success())
}

pub fn remove_dynamic_port(ifname: &str) -> Result<()> {
    let port_name = format!("dyn-port-{ifname}");
    let eth_name = eth_conn_name(ifname);
    
    info!("Removing dynamic OVS port {} for interface {}", port_name, ifname);
    
    // Deactivate ethernet connection first (this will deactivate the port as well)
    if connection_exists(&eth_name)? {
        debug!("Deactivating ethernet connection {}", eth_name);
        let output = Command::new("nmcli")
            .args(["connection", "down", &eth_name])
            .output()
            .context("Failed to deactivate ethernet connection")?;
        
        if !output.status.success() {
            warn!("Failed to deactivate {}: {}", eth_name, String::from_utf8_lossy(&output.stderr));
        }
        
        // Delete ethernet connection
        debug!("Deleting ethernet connection {}", eth_name);
        let output = Command::new("nmcli")
            .args(["connection", "delete", &eth_name])
            .output()
            .context("Failed to delete ethernet connection")?;
        
        if !output.status.success() {
            warn!("Failed to delete {}: {}", eth_name, String::from_utf8_lossy(&output.stderr));
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
            warn!("Failed to delete {}: {}", port_name, String::from_utf8_lossy(&output.stderr));
        }
    }
    
    info!("Successfully removed dynamic port for interface {}", ifname);
    Ok(())
}
