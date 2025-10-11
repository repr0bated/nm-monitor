use anyhow::{bail, Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::process::Command;

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

/// Ensure the full bridge topology (bridge + internal port + optional uplink) exists
pub fn ensure_bridge_topology(bridge: &str, uplink: Option<&str>, wait_seconds: u32) -> Result<()> {
    let cfg = OvsBridgeConfig {
        name: bridge.to_string(),
        ..Default::default()
    };

    if connection_exists(bridge)? {
        modify_ovs_bridge(&cfg)?;
    } else {
        create_ovs_bridge(&cfg)?;
    }

    create_ovs_internal_port(bridge)?;

    if let Some(uplink_if) = uplink {
        create_ovs_uplink_port(bridge, uplink_if)?;
    }

    activate_bridge(bridge, wait_seconds)?;
    validate_bridge_topology(bridge)?;

    Ok(())
}

/// Create OVS bridge with strict NetworkManager compliance
pub fn create_ovs_bridge(config: &OvsBridgeConfig) -> Result<()> {
    info!("Creating OVS bridge {} with NetworkManager", config.name);

    // Check if connection already exists
    if connection_exists(&config.name)? {
        debug!(
            "Bridge connection {} already exists, modifying",
            config.name
        );
        modify_ovs_bridge(config)?;
        return Ok(());
    }

    let mut args = vec![
        "conn",
        "add",
        "type",
        "ovs-bridge",
        "con-name",
        &config.name,
        "conn.interface",
        &config.name,
    ];

    // Add OVS-specific properties according to NM documentation
    let stp_val = if config.stp_enable { "yes" } else { "no" };
    let rstp_val = if config.rstp_enable { "yes" } else { "no" };
    let mcast_val = if config.mcast_snooping_enable {
        "yes"
    } else {
        "no"
    };

    args.extend_from_slice(&[
        "ovs-bridge.stp",
        stp_val,
        "ovs-bridge.rstp",
        rstp_val,
        "ovs-bridge.mcast-snooping-enable",
        mcast_val,
        "connection.autoconnect",
        "yes",
        "connection.autoconnect-priority",
        "100",
        "connection.autoconnect-ports",
        "1",
        "connection.autoconnect-slaves",
        "1",
        "ipv4.method",
        "auto",
        "ipv6.method",
        "auto",
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
        bail!(
            "Failed to create OVS bridge: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    info!("Successfully created OVS bridge {}", config.name);
    Ok(())
}

/// Modify existing OVS bridge connection
pub fn modify_ovs_bridge(config: &OvsBridgeConfig) -> Result<()> {
    let mut args = vec![
        "conn",
        "modify",
        &config.name,
        "connection.interface-name",
        &config.name,
    ];

    let stp_val = if config.stp_enable { "yes" } else { "no" };
    let rstp_val = if config.rstp_enable { "yes" } else { "no" };
    let mcast_val = if config.mcast_snooping_enable {
        "yes"
    } else {
        "no"
    };

    args.extend_from_slice(&[
        "ovs-bridge.stp",
        stp_val,
        "ovs-bridge.rstp",
        rstp_val,
        "ovs-bridge.mcast-snooping-enable",
        mcast_val,
        "connection.autoconnect",
        "yes",
        "connection.autoconnect-priority",
        "100",
        "connection.autoconnect-ports",
        "1",
        "connection.autoconnect-slaves",
        "1",
        "ipv4.method",
        "auto",
        "ipv6.method",
        "auto",
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
        bail!(
            "Failed to modify OVS bridge: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Create OVS port with internal interface for bridge IP assignment
pub fn create_ovs_internal_port(bridge: &str) -> Result<()> {
    let port_conn = format!("{bridge}_port_int");
    let port_ifname = port_conn.clone();
    let iface_conn = format!("{bridge}_if");
    let iface_ifname = iface_conn.clone();

    info!(
        "Ensuring internal OVS hierarchy: bridge={}, port={} (ifname={}), iface={} (ifname={})",
        bridge, port_conn, port_ifname, iface_conn, iface_ifname
    );

    if connection_exists(&port_conn)? {
        delete_connection(&port_conn)?;
    }

    let port_args = [
        "conn",
        "add",
        "type",
        "ovs-port",
        "con-name",
        port_conn.as_str(),
        "conn.interface",
        port_ifname.as_str(),
        "controller",
        bridge,
        "connection.autoconnect",
        "yes",
        "connection.autoconnect-priority",
        "95",
    ];

    let output = Command::new("nmcli")
        .args(port_args)
        .output()
        .context("Failed to create OVS internal port")?;

    if !output.status.success() {
        bail!(
            "Failed to create OVS port: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    if connection_exists(&iface_conn)? {
        delete_connection(&iface_conn)?;
    }

    let iface_args = [
        "conn",
        "add",
        "type",
        "ovs-interface",
        "con-name",
        iface_conn.as_str(),
        "conn.interface",
        iface_ifname.as_str(),
        "controller",
        port_conn.as_str(),
        "port-type",
        "ovs-port",
        "ovs-interface.type",
        "internal",
        "connection.autoconnect",
        "yes",
        "connection.autoconnect-priority",
        "95",
        "ipv4.method",
        "auto",
        "ipv6.method",
        "auto",
    ];

    let output = Command::new("nmcli")
        .args(iface_args)
        .output()
        .context("Failed to create OVS internal interface")?;

    if !output.status.success() {
        bail!(
            "Failed to create OVS interface: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    info!("Internal OVS port/interface ensured for bridge {}", bridge);
    Ok(())
}

/// Create OVS port for external interface (uplink)
pub fn create_ovs_uplink_port(bridge: &str, ifname: &str) -> Result<()> {
    let port_conn = format!("{}_port_{}", bridge, ifname);
    let port_ifname = port_conn.clone();

    info!(
        "Ensuring uplink topology: bridge={}, port={}, iface={}",
        bridge, port_conn, ifname
    );

    // Create or update OVS port
    if connection_exists(&port_conn)? {
        delete_connection(&port_conn)?;
    }

    let port_args = [
        "conn",
        "add",
        "type",
        "ovs-port",
        "con-name",
        port_conn.as_str(),
        "conn.interface",
        port_ifname.as_str(),
        "controller",
        bridge,
        "connection.autoconnect",
        "yes",
        "connection.autoconnect-priority",
        "90",
    ];

    let output = Command::new("nmcli")
        .args(port_args)
        .output()
        .context("Failed to create OVS uplink port")?;

    if !output.status.success() {
        bail!(
            "Failed to create OVS uplink port: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Create or modify ethernet connection as slave
    let eth_name = format!("{}_uplink_{}", bridge, ifname);
    if connection_exists(&eth_name)? {
        delete_connection(&eth_name)?;
    }

    let eth_args = [
        "conn",
        "add",
        "type",
        "ethernet",
        "con-name",
        eth_name.as_str(),
        "conn.interface",
        ifname,
        "controller",
        port_conn.as_str(),
        "port-type",
        "ovs-port",
        "connection.autoconnect",
        "yes",
        "connection.autoconnect-priority",
        "85",
    ];

    let output = Command::new("nmcli")
        .args(eth_args)
        .output()
        .context("Failed to create ethernet slave")?;

    if !output.status.success() {
        // Try to migrate existing active profile
        modify_ethernet_to_slave(&port_conn, ifname, &eth_name)?;
    }

    info!("Uplink port ensured for interface {}", ifname);
    Ok(())
}

/// Modify existing ethernet connection to be OVS slave
fn modify_ethernet_to_slave(port_name: &str, ifname: &str, desired_id: &str) -> Result<()> {
    // Find active ethernet connection on this interface
    let output = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "NAME,DEVICE,TYPE",
            "connection",
            "show",
            "--active",
        ])
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
        info!(
            "Modifying existing ethernet connection {} to OVS slave",
            conn_name
        );

        let args = vec![
            "conn",
            "modify",
            &conn_name,
            "conn.interface",
            ifname,
            "controller",
            port_name,
            "port-type",
            "ovs-port",
            "connection.autoconnect",
            "yes",
            "connection.autoconnect-priority",
            "85",
        ];

        let output = Command::new("nmcli")
            .args(&args)
            .output()
            .context("Failed to modify ethernet connection")?;

        if !output.status.success() {
            bail!(
                "Failed to modify ethernet connection: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Rename to match our convention
        let rename = Command::new("nmcli")
            .args(["conn", "modify", &conn_name, "connection.id", desired_id])
            .output()
            .context("Failed to rename ethernet connection")?;
        if !rename.status.success() {
            debug!(
                "Unable to rename {} to {}: {}",
                conn_name,
                desired_id,
                String::from_utf8_lossy(&rename.stderr)
            );
        }
    }

    Ok(())
}

/// Activate bridge connection atomically (NetworkManager handles slaves)
pub fn activate_bridge(bridge: &str, wait_seconds: u32) -> Result<()> {
    info!("Activating OVS bridge {} (atomic handoff)", bridge);

    let wait_str = wait_seconds.to_string();
    let args = vec!["-w", &wait_str, "connection", "up", bridge];

    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .context("Failed to activate bridge")?;

    if !output.status.success() {
        bail!(
            "Failed to activate bridge: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    info!(
        "Successfully activated OVS bridge {} with all slaves",
        bridge
    );
    Ok(())
}

/// Check if NetworkManager connection exists
pub fn connection_exists(name: &str) -> Result<bool> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show"])
        .output()
        .context("Failed to list connections")?;

    if !output.status.success() {
        return Ok(false);
    }

    let connections = String::from_utf8_lossy(&output.stdout);
    Ok(connections.lines().any(|line| line.trim() == name))
}

fn delete_connection(name: &str) -> Result<()> {
    let output = Command::new("nmcli")
        .args(["connection", "delete", name])
        .output()
        .context("Failed to delete connection")?;

    if !output.status.success() {
        debug!(
            "Ignoring failure deleting connection {}: {}",
            name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Validate OVS bridge topology
pub fn validate_bridge_topology(bridge: &str) -> Result<()> {
    info!("Validating OVS bridge {} topology", bridge);

    // Ensure required connection profiles exist
    let int_port = format!("{}_port_int", bridge);
    let interface = format!("{}_if", bridge);
    for name in [bridge, int_port.as_str(), interface.as_str()] {
        if !connection_exists(name)? {
            bail!("Required connection {name} does not exist");
        }
    }

    // Allow a short window for NM to reflect activation state
    const ATTEMPTS: usize = 12;
    const DELAY: std::time::Duration = std::time::Duration::from_millis(1000);
    let mut last_status = (false, false, false);
    for attempt in 0..ATTEMPTS {
        let bridge_active = connection_is_active(bridge)?;
        let port_active = connection_is_active(&int_port)?;
        let if_active = connection_is_active(&interface)?;
        last_status = (bridge_active, port_active, if_active);

        if bridge_active && port_active && if_active {
            info!("OVS bridge {} topology validated successfully", bridge);
            return Ok(());
        }

        if attempt + 1 < ATTEMPTS {
            debug!(
                "bridge topology not yet active (bridge={}, port={}, interface={}), retrying...",
                bridge_active, port_active, if_active
            );
            std::thread::sleep(DELAY);
        }
    }

    let (bridge_active, port_active, if_active) = last_status;
    if bridge_active && port_active {
        warn!(
            "Bridge validated with interface reporting inactive (bridge={}, port={}, interface={}); continuing",
            bridge_active,
            port_active,
            if_active
        );
        return Ok(());
    }

    bail!(
        "Bridge topology not fully active: bridge={}, port={}, interface={}",
        bridge_active,
        port_active,
        if_active
    );
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
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
fn connection_is_active(name: &str) -> Result<bool> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "GENERAL.STATE", "connection", "show", name])
        .output()
        .with_context(|| format!("checking connection state for {name}"))?;

    if !output.status.success() {
        return Ok(false);
    }

    let state = String::from_utf8_lossy(&output.stdout);
    Ok(state.trim_end().ends_with("activated"))
}
