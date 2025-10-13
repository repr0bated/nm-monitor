use anyhow::{bail, Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

/// Systemd-networkd OVS Bridge configuration
/// Strictly follows systemd.network documentation
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

    if bridge_exists(bridge)? {
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

/// Create OVS bridge with systemd-networkd .netdev and .network files
pub fn create_ovs_bridge(config: &OvsBridgeConfig) -> Result<()> {
    info!("Creating OVS bridge {} with systemd-networkd", config.name);

    // Check if bridge already exists
    if bridge_exists(&config.name)? {
        debug!("Bridge {} already exists, modifying", config.name);
        modify_ovs_bridge(config)?;
        return Ok(());
    }

    // Create .netdev file for the OVS bridge
    create_bridge_netdev(config)?;

    // Create .network file for the bridge
    create_bridge_network(config)?;

    info!("Successfully created OVS bridge {}", config.name);
    Ok(())
}

/// Modify existing OVS bridge configuration
pub fn modify_ovs_bridge(config: &OvsBridgeConfig) -> Result<()> {
    // Recreate the configuration files
    create_bridge_netdev(config)?;
    create_bridge_network(config)?;

    // Reload systemd-networkd
    reload_networkd()?;

    Ok(())
}

/// Create .netdev file for OVS bridge
fn create_bridge_netdev(config: &OvsBridgeConfig) -> Result<()> {
    let mut netdev_content = format!(
        "[NetDev]\n\
         Name={}\n\
         Kind=ovs-bridge\n",
        config.name
    );

    if config.stp_enable {
        netdev_content.push_str("OVSBridge.STP=yes\n");
    } else {
        netdev_content.push_str("OVSBridge.STP=no\n");
    }

    if config.rstp_enable {
        netdev_content.push_str("OVSBridge.RSTP=yes\n");
    } else {
        netdev_content.push_str("OVSBridge.RSTP=no\n");
    }

    if config.mcast_snooping_enable {
        netdev_content.push_str("OVSBridge.McastSnooping=yes\n");
    } else {
        netdev_content.push_str("OVSBridge.McastSnooping=no\n");
    }

    if let Some(ref dt) = config.datapath_type {
        netdev_content.push_str(&format!("OVSBridge.DatapathType={}\n", dt));
    }

    let netdev_path = format!("/etc/systemd/network/{}.netdev", config.name);
    fs::write(&netdev_path, netdev_content)
        .with_context(|| format!("writing .netdev file for bridge {}", config.name))?;

    Ok(())
}

/// Create .network file for OVS bridge
fn create_bridge_network(config: &OvsBridgeConfig) -> Result<()> {
    let network_content = format!(
        "[Match]\n\
         Name={}\n\
         \n\
         [Network]\n\
         DHCP=yes\n\
         IPv6AcceptRA=yes\n",
        config.name
    );

    let network_path = format!("/etc/systemd/network/{}.network", config.name);
    fs::write(&network_path, network_content)
        .with_context(|| format!("writing .network file for bridge {}", config.name))?;

    Ok(())
}

/// Create OVS port with internal interface
pub fn create_ovs_internal_port(bridge: &str) -> Result<()> {
    let iface_name = format!("{}_if", bridge);

    info!(
        "Ensuring internal OVS interface: bridge={}, iface={}",
        bridge, iface_name
    );

    // Create .netdev for internal interface
    let port_netdev = format!(
        "[NetDev]\n\
         Name={}\n\
         Kind=ovs-interface\n\
         \n\
         [OVSInterface]\n\
         Type=internal\n\
         Bridge={}\n",
        iface_name, bridge
    );

    let port_netdev_path = format!("/etc/systemd/network/{}.netdev", iface_name);
    fs::write(&port_netdev_path, port_netdev)
        .with_context(|| format!("writing internal interface .netdev for {}", iface_name))?;

    // Create .network for internal interface
    let port_network = format!(
        "[Match]\n\
         Name={}\n\
         \n\
         [Network]\n\
         DHCP=yes\n\
         IPv6AcceptRA=yes\n",
        iface_name
    );

    let port_network_path = format!("/etc/systemd/network/{}.network", iface_name);
    fs::write(&port_network_path, port_network)
        .with_context(|| format!("writing internal interface .network for {}", iface_name))?;

    // Reload networkd
    reload_networkd()?;

    info!("Internal OVS interface ensured for bridge {}", bridge);
    Ok(())
}

/// Create OVS port for external interface (uplink)
pub fn create_ovs_uplink_port(bridge: &str, ifname: &str) -> Result<()> {
    info!(
        "Ensuring uplink topology: bridge={}, iface={}",
        bridge, ifname
    );

    // Create .network file for the uplink interface to connect to OVS bridge
    let uplink_network = format!(
        "[Match]\n\
         Name={}\n\
         \n\
         [Network]\n\
         Bridge={}\n",
        ifname, bridge
    );

    let uplink_network_path = format!("/etc/systemd/network/50-{}.network", ifname);
    fs::write(&uplink_network_path, uplink_network)
        .with_context(|| format!("writing uplink .network for {}", ifname))?;

    // Reload networkd
    reload_networkd()?;

    info!("Uplink ensured for interface {}", ifname);
    Ok(())
}

/// Activate bridge using systemctl
pub fn activate_bridge(bridge: &str, wait_seconds: u32) -> Result<()> {
    info!("Activating OVS bridge {} with systemd-networkd", bridge);

    // Restart systemd-networkd to pick up new configurations
    let output = Command::new("systemctl")
        .args(["restart", "systemd-networkd"])
        .output()
        .context("Failed to restart systemd-networkd")?;

    if !output.status.success() {
        bail!(
            "Failed to restart systemd-networkd: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Wait for the bridge to come up
    std::thread::sleep(std::time::Duration::from_secs(wait_seconds as u64));

    // Check if bridge is active
    if !bridge_exists(bridge)? {
        bail!("Bridge {} did not come up after restart", bridge);
    }

    info!("Successfully activated OVS bridge {}", bridge);
    Ok(())
}

/// Check if OVS bridge exists
pub fn bridge_exists(name: &str) -> Result<bool> {
    let output = Command::new("networkctl")
        .args(["list", "--no-pager"])
        .output()
        .context("Failed to list network interfaces")?;

    if !output.status.success() {
        return Ok(false);
    }

    let networks = String::from_utf8_lossy(&output.stdout);
    Ok(networks.lines().any(|line| line.contains(name)))
}

/// Reload systemd-networkd configuration
fn reload_networkd() -> Result<()> {
    let output = Command::new("networkctl")
        .args(["reload"])
        .output()
        .context("Failed to reload systemd-networkd")?;

    if !output.status.success() {
        warn!(
            "Failed to reload systemd-networkd: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Validate OVS bridge topology
pub fn validate_bridge_topology(bridge: &str) -> Result<()> {
    info!("Validating OVS bridge {} topology", bridge);

    // Check if bridge exists in systemd-networkd
    if !bridge_exists(bridge)? {
        bail!("Bridge {} does not exist in systemd-networkd", bridge);
    }

    // Check OVS level
    let output = Command::new("ovs-vsctl")
        .args(["br-exists", bridge])
        .output()
        .context("Failed to check OVS bridge existence")?;

    if !output.status.success() {
        bail!("OVS bridge {} does not exist", bridge);
    }

    // Check if bridge has ports
    let output = Command::new("ovs-vsctl")
        .args(["list-ports", bridge])
        .output()
        .context("Failed to list bridge ports")?;

    let ports = String::from_utf8_lossy(&output.stdout);
    if !ports.lines().any(|line| !line.trim().is_empty()) {
        warn!("Bridge {} has no ports configured", bridge);
    }

    info!("OVS bridge {} topology validated successfully", bridge);
    Ok(())
}

/// Remove bridge configuration files
#[allow(dead_code)]
pub fn remove_bridge(bridge: &str) -> Result<()> {
    info!("Removing OVS bridge {} configuration", bridge);

    // Remove .netdev and .network files
    let files_to_remove = vec![
        format!("/etc/systemd/network/{}.netdev", bridge),
        format!("/etc/systemd/network/{}.network", bridge),
        format!("/etc/systemd/network/{}_if.netdev", bridge),
        format!("/etc/systemd/network/{}_if.network", bridge),
    ];

    for file in files_to_remove {
        if std::path::Path::new(&file).exists() {
            fs::remove_file(&file).with_context(|| format!("removing config file {}", file))?;
        }
    }

    // Reload networkd
    reload_networkd()?;

    info!("Successfully removed bridge {} configuration", bridge);
    Ok(())
}
