use anyhow::{Context, Result, bail};
use std::process::Command;
use log::{info, debug};
use serde::{Serialize, Deserialize};

/// NetworkManager OVS Bridge configuration
/// Strictly follows NetworkManager.dev documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsBridgeConfig {
    pub name: String,
    pub stp_enable: bool,
    pub rstp_enable: bool,
    pub mcast_snooping_enable: bool,
    pub datapath_type: Option<String>,
    pub fail_mode: Option<String>,
}

impl Default for OvsBridgeConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            stp_enable: false,
            rstp_enable: false,
            mcast_snooping_enable: true,
            datapath_type: None,
            fail_mode: None,
        }
    }
}

/// NetworkManager OVS Port configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsPortConfig {
    pub name: String,
    pub bridge: String,
    pub tag: Option<u32>,
    pub vlan_mode: Option<String>,
    pub lacp: Option<String>,
    pub bond_mode: Option<String>,
    pub bond_updelay: Option<u32>,
    pub bond_downdelay: Option<u32>,
}

/// NetworkManager OVS Interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsInterfaceConfig {
    pub name: String,
    pub type_: String,
    pub ofport_request: Option<u32>,
}

/// Create OVS bridge with strict NetworkManager compliance
pub fn create_ovs_bridge(config: &OvsBridgeConfig) -> Result<()> {
    info!("Creating OVS bridge {} with NetworkManager", config.name);
    
    // Check if connection already exists
    if connection_exists(&config.name)? {
        debug!("Bridge connection {} already exists, modifying", config.name);
        modify_ovs_bridge(config)?;
        return Ok(());
    }
    
    let mut args = vec![
        "connection", "add",
        "type", "ovs-bridge",
        "con-name", &config.name,
        "ifname", &config.name,
    ];
    
    // Add OVS-specific properties according to NM documentation
    let stp_val = if config.stp_enable { "yes" } else { "no" };
    let rstp_val = if config.rstp_enable { "yes" } else { "no" };
    let mcast_val = if config.mcast_snooping_enable { "yes" } else { "no" };
    
    args.extend_from_slice(&[
        "ovs-bridge.stp", stp_val,
        "ovs-bridge.rstp", rstp_val,
        "ovs-bridge.mcast-snooping-enable", mcast_val,
    ]);
    
    if let Some(ref datapath_type) = config.datapath_type {
        args.extend_from_slice(&["ovs-bridge.datapath-type", datapath_type]);
    }
    
    if let Some(ref fail_mode) = config.fail_mode {
        args.extend_from_slice(&["ovs-bridge.fail-mode", fail_mode]);
    }
    
    // Set connection properties for autoconnect
    args.extend_from_slice(&[
        "connection.autoconnect", "yes",
        "connection.autoconnect-priority", "100",
    ]);
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to execute nmcli")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS bridge: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    info!("Successfully created OVS bridge {}", config.name);
    Ok(())
}

/// Modify existing OVS bridge connection
pub fn modify_ovs_bridge(config: &OvsBridgeConfig) -> Result<()> {
    let mut args = vec![
        "connection", "modify", &config.name,
    ];
    
    let stp_val = if config.stp_enable { "yes" } else { "no" };
    let rstp_val = if config.rstp_enable { "yes" } else { "no" };
    let mcast_val = if config.mcast_snooping_enable { "yes" } else { "no" };
    
    args.extend_from_slice(&[
        "ovs-bridge.stp", stp_val,
        "ovs-bridge.rstp", rstp_val,
        "ovs-bridge.mcast-snooping-enable", mcast_val,
    ]);
    
    if let Some(ref datapath_type) = config.datapath_type {
        args.extend_from_slice(&["ovs-bridge.datapath-type", datapath_type]);
    }
    
    if let Some(ref fail_mode) = config.fail_mode {
        args.extend_from_slice(&["ovs-bridge.fail-mode", fail_mode]);
    }
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to execute nmcli")?;
    
    if !output.status.success() {
        bail!("Failed to modify OVS bridge: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(())
}

/// Create OVS port with internal interface for bridge IP assignment
pub fn create_ovs_internal_port(bridge: &str) -> Result<()> {
    let port_name = format!("{}-port-int", bridge);
    let if_name = bridge; // Internal port uses bridge name as interface
    
    info!("Creating OVS internal port {} for bridge {}", port_name, bridge);
    
    // Create OVS port
    let args = vec![
        "connection", "add",
        "type", "ovs-port",
        "con-name", &port_name,
        "ifname", if_name,
        "connection.master", bridge,
        "connection.slave-type", "ovs-bridge",
        "connection.autoconnect", "yes",
        "connection.autoconnect-priority", "95",
    ];
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to create OVS port")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS port: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Create OVS interface
    let if_con_name = format!("{}-if", bridge);
    let args = vec![
        "connection", "add",
        "type", "ovs-interface",
        "con-name", &if_con_name,
        "ifname", if_name,
        "connection.master", &port_name,
        "connection.slave-type", "ovs-port",
        "connection.autoconnect", "yes",
        "connection.autoconnect-priority", "95",
        "ovs-interface.type", "internal",
    ];
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to create OVS interface")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS interface: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    info!("Successfully created OVS internal port for bridge {}", bridge);
    Ok(())
}

/// Create OVS port for external interface (uplink)
pub fn create_ovs_uplink_port(bridge: &str, ifname: &str) -> Result<()> {
    let port_name = format!("{}-port-{}", bridge, ifname);
    
    info!("Creating OVS uplink port {} for interface {}", port_name, ifname);
    
    // Create OVS port
    let args = vec![
        "connection", "add",
        "type", "ovs-port",
        "con-name", &port_name,
        "ifname", ifname,
        "connection.master", bridge,
        "connection.slave-type", "ovs-bridge",
        "connection.autoconnect", "yes",
        "connection.autoconnect-priority", "90",
    ];
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to create OVS uplink port")?;
    
    if !output.status.success() {
        bail!("Failed to create OVS uplink port: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Create or modify ethernet connection as slave
    let eth_name = format!("{}-eth-{}", bridge, ifname);
    let args = vec![
        "connection", "add",
        "type", "ethernet",
        "con-name", &eth_name,
        "ifname", ifname,
        "connection.master", &port_name,
        "connection.slave-type", "ovs-port",
        "connection.autoconnect", "yes",
        "connection.autoconnect-priority", "85",
    ];
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to create ethernet slave")?;
    
    if !output.status.success() {
        // Try to modify existing connection
        modify_ethernet_to_slave(&port_name, ifname)?;
    }
    
    info!("Successfully created OVS uplink port for {}", ifname);
    Ok(())
}

/// Modify existing ethernet connection to be OVS slave
fn modify_ethernet_to_slave(port_name: &str, ifname: &str) -> Result<()> {
    // Find active ethernet connection on this interface
    let output = Command::new("nmcli")
        .args(&["-t", "-f", "NAME,DEVICE,TYPE", "connection", "show", "--active"])
        .output()
        .context("Failed to list active connections")?;
    
    let active_conns = String::from_utf8_lossy(&output.stdout);
    let mut eth_conn_name = None;
    
    for line in active_conns.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && parts[1] == ifname && parts[2] == "802-3-ethernet" {
            eth_conn_name = Some(parts[0].to_string());
            break;
        }
    }
    
    if let Some(conn_name) = eth_conn_name {
        info!("Modifying existing ethernet connection {} to OVS slave", conn_name);
        
        let args = vec![
            "connection", "modify", &conn_name,
            "connection.master", port_name,
            "connection.slave-type", "ovs-port",
            "connection.autoconnect", "yes",
            "connection.autoconnect-priority", "85",
        ];
        
        let output = Command::new("nmcli")
            .args(&args)
            .output()
            .context("Failed to modify ethernet connection")?;
        
        if !output.status.success() {
            bail!("Failed to modify ethernet connection: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
    
    Ok(())
}

/// Activate bridge connection atomically (NetworkManager handles slaves)
pub fn activate_bridge(bridge: &str, wait_seconds: u32) -> Result<()> {
    info!("Activating OVS bridge {} (atomic handoff)", bridge);
    
    let args = vec![
        "-w", &wait_seconds.to_string(),
        "connection", "up", bridge,
    ];
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to activate bridge")?;
    
    if !output.status.success() {
        bail!("Failed to activate bridge: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    info!("Successfully activated OVS bridge {} with all slaves", bridge);
    Ok(())
}

/// Check if NetworkManager connection exists
pub fn connection_exists(name: &str) -> Result<bool> {
    let output = Command::new("nmcli")
        .args(&["-t", "-f", "NAME", "connection", "show"])
        .output()
        .context("Failed to list connections")?;
    
    if !output.status.success() {
        return Ok(false);
    }
    
    let connections = String::from_utf8_lossy(&output.stdout);
    Ok(connections.lines().any(|line| line.trim() == name))
}

/// Configure IP address on OVS interface
pub fn configure_ip_address(if_name: &str, ip_addr: &str, gateway: Option<&str>) -> Result<()> {
    info!("Configuring IP {} on interface {}", ip_addr, if_name);
    
    let mut args = vec![
        "connection", "modify", if_name,
        "ipv4.method", "manual",
        "ipv4.addresses", ip_addr,
        "ipv6.method", "disabled",
    ];
    
    if let Some(gw) = gateway {
        args.extend_from_slice(&["ipv4.gateway", gw]);
    }
    
    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to configure IP address")?;
    
    if !output.status.success() {
        bail!("Failed to configure IP: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    Ok(())
}

/// Get D-Bus introspection data for NetworkManager
pub async fn get_nm_introspection() -> Result<String> {
    use zbus::fdo::IntrospectableProxy;
    
    let conn = zbus::Connection::system().await?;
    let proxy = IntrospectableProxy::builder(&conn)
        .destination("org.freedesktop.NetworkManager")?
        .path("/org/freedesktop/NetworkManager")?
        .build()
        .await?;
    
    let xml = proxy.introspect().await?;
    Ok(xml)
}

/// Validate OVS bridge topology
pub fn validate_bridge_topology(bridge: &str) -> Result<()> {
    info!("Validating OVS bridge {} topology", bridge);
    
    // Check bridge connection exists
    if !connection_exists(bridge)? {
        bail!("Bridge connection {} does not exist", bridge);
    }
    
    // Check internal port
    let int_port = format!("{}-port-int", bridge);
    if !connection_exists(&int_port)? {
        bail!("Internal port {} does not exist", int_port);
    }
    
    // Check interface
    let interface = format!("{}-if", bridge);
    if !connection_exists(&interface)? {
        bail!("Interface connection {} does not exist", interface);
    }
    
    // Verify connections are active
    let output = Command::new("nmcli")
        .args(&["-t", "-f", "NAME,STATE", "connection", "show"])
        .output()
        .context("Failed to check connection states")?;
    
    let states = String::from_utf8_lossy(&output.stdout);
    let mut bridge_active = false;
    let mut port_active = false;
    let mut if_active = false;
    
    for line in states.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 2 {
            match parts[0] {
                name if name == bridge => bridge_active = parts[1] == "activated",
                name if name == int_port => port_active = parts[1] == "activated",
                name if name == interface => if_active = parts[1] == "activated",
                _ => {}
            }
        }
    }
    
    if !bridge_active || !port_active || !if_active {
        bail!("Bridge topology not fully active: bridge={}, port={}, interface={}", 
              bridge_active, port_active, if_active);
    }
    
    info!("OVS bridge {} topology validated successfully", bridge);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ovs_bridge_config_default() {
        let config = OvsBridgeConfig::default();
        assert!(!config.stp_enable);
        assert!(!config.rstp_enable);
        assert!(config.mcast_snooping_enable);
        assert!(config.datapath_type.is_none());
        assert!(config.fail_mode.is_none());
    }
}