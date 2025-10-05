use anyhow::{Context, Result, bail};
use std::process::Command;
use log::{info, debug};

/// Create OVS bridge following NetworkManager documentation exactly
/// Using controller relationship instead of deprecated master/slave
pub fn create_ovs_bridge_controlled(bridge_name: &str) -> Result<()> {
    info!("Creating OVS bridge {} (Example 20)", bridge_name);
    
    // nmcli conn add type ovs-bridge conn.interface bridge0
    let output = Command::new("nmcli")
        .args([
            "conn", "add",
            "type", "ovs-bridge",
            "conn.interface", bridge_name,
        ])
        .output()
        .context("Failed to execute nmcli")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS bridge: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    info!("Successfully created OVS bridge {}", bridge_name);
    Ok(())
}

/// Create OVS port with controller relationship
pub fn create_ovs_port_controlled(port_name: &str, controller: &str) -> Result<()> {
    info!("Creating OVS port {} controlled by {}", port_name, controller);
    
    // nmcli conn add type ovs-port conn.interface port0 controller bridge0
    let output = Command::new("nmcli")
        .args([
            "conn", "add",
            "type", "ovs-port",
            "conn.interface", port_name,
            "controller", controller,
        ])
        .output()
        .context("Failed to execute nmcli")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS port: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(())
}

/// Create OVS interface with IP configuration
pub fn create_ovs_interface_controlled(
    if_name: &str, 
    controller: &str,
    ip_addr: Option<&str>,
    gateway: Option<&str>
) -> Result<()> {
    info!("Creating OVS interface {} controlled by {}", if_name, controller);
    
    let mut args = vec![
        "conn", "add",
        "type", "ovs-interface",
        "port-type", "ovs-port",
        "conn.interface", if_name,
        "controller", controller,
    ];
    
    // Add IP configuration if provided
    if let Some(ip) = ip_addr {
        args.extend_from_slice(&["ipv4.method", "manual", "ipv4.address", ip]);
        if let Some(gw) = gateway {
            args.extend_from_slice(&["ipv4.gateway", gw]);
        }
    } else {
        args.extend_from_slice(&["ipv4.method", "disabled"]);
    }
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to execute nmcli")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS interface: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(())
}

/// Add Linux interface to bridge (Example 21)
pub fn add_ethernet_to_bridge(ifname: &str, controller: &str) -> Result<()> {
    info!("Adding ethernet {} to bridge via {}", ifname, controller);
    
    // nmcli conn add type ethernet conn.interface eth0 controller port1
    let output = Command::new("nmcli")
        .args([
            "conn", "add",
            "type", "ethernet",
            "conn.interface", ifname,
            "controller", controller,
        ])
        .output()
        .context("Failed to execute nmcli")?;
    
    if !output.status.success() {
        bail!("Failed to add ethernet interface: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(())
}

/// Create complete OVS bridge setup following NetworkManager documentation
pub fn setup_ovs_bridge_complete(
    bridge_name: &str,
    ip_addr: Option<&str>,
    gateway: Option<&str>,
    uplink: Option<&str>,
) -> Result<()> {
    // Example 20: Creating a Bridge with a single internal Interface
    
    // Step 1: Create bridge
    create_ovs_bridge_controlled(bridge_name)?;
    
    // Step 2: Create port
    create_ovs_port_controlled("port0", bridge_name)?;
    
    // Step 3: Create interface with IP
    create_ovs_interface_controlled("iface0", "port0", ip_addr, gateway)?;
    
    // Example 21: Adding a Linux interface to a Bridge
    if let Some(uplink_if) = uplink {
        // Create port for uplink
        create_ovs_port_controlled("port1", bridge_name)?;
        
        // Add ethernet interface
        add_ethernet_to_bridge(uplink_if, "port1")?;
    }
    
    info!("OVS bridge setup complete. Check with: ovs-vsctl show");
    Ok(())
}

/// List OVS connections using proper controller terminology
pub fn list_ovs_connections() -> Result<Vec<String>> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
        .output()
        .context("Failed to list connections")?;
    
    if !output.status.success() {
        return Ok(vec![]);
    }
    
    let connections = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains("ovs-"))
        .map(|line| line.split(':').next().unwrap_or("").to_string())
        .collect();
    
    Ok(connections)
}