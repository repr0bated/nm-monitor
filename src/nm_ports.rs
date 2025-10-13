use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::fs;
use std::process::Command;

/// Ensure a proactive OVS port exists for an interface using systemd-networkd
pub fn ensure_proactive_port(bridge: &str, ifname: &str) -> Result<()> {
    let port_name = format!("ovs-port-{}", ifname);

    info!(
        "Ensuring proactive OVS port {} on bridge {} for interface {}",
        port_name, bridge, ifname
    );

    if port_exists(&port_name)? {
        debug!(
            "OVS port {} already exists and is active, skipping",
            port_name
        );
        return Ok(());
    }

    // Remove any existing configuration files
    remove_port_config(&port_name, &format!("ovs-eth-{}", ifname))?;

    debug!("Creating OVS port {} on bridge {}", port_name, bridge);

    // Create .netdev file for the OVS port
    let port_netdev = format!(
        "[NetDev]\n\
         Name={}\n\
         Kind=ovs-interface\n\
         \n\
         [OVSInterface]\n\
         Type=system\n\
         Bridge={}\n",
        port_name, bridge
    );

    let port_netdev_path = format!("/etc/systemd/network/{}.netdev", port_name);
    fs::write(&port_netdev_path, port_netdev)
        .with_context(|| format!("writing port .netdev for {}", port_name))?;

    // Create .network file for the ethernet interface
    let eth_name = format!("ovs-eth-{}", ifname);
    let eth_network = format!(
        "[Match]\n\
         Name={}\n\
         \n\
         [Network]\n\
         Bridge={}\n",
        ifname, bridge
    );

    let eth_network_path = format!("/etc/systemd/network/{}.network", eth_name);
    fs::write(&eth_network_path, eth_network)
        .with_context(|| format!("writing ethernet .network for {}", eth_name))?;

    // Reload networkd
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

    info!(
        "Successfully ensured proactive port {} on bridge {}",
        ifname, bridge
    );
    Ok(())
}

/// Remove proactive OVS port configuration
pub fn remove_proactive_port(port_name: &str, eth_name: &str) -> Result<()> {
    info!(
        "Removing proactive OVS port {} and ethernet {}",
        port_name, eth_name
    );

    remove_port_config(port_name, eth_name)?;

    // Reload networkd
    let output = Command::new("networkctl")
        .args(["reload"])
        .output()
        .context("Failed to reload systemd-networkd after port removal")?;

    if !output.status.success() {
        warn!(
            "Failed to reload systemd-networkd: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    info!("Successfully removed proactive port connections");
    Ok(())
}

/// Check if a network interface exists in systemd-networkd
fn port_exists(name: &str) -> Result<bool> {
    let output = Command::new("networkctl")
        .args(["list", "--no-pager", "--no-legend"])
        .output()
        .context("Failed to list network interfaces")?;

    if !output.status.success() {
        return Ok(false);
    }

    let networks = String::from_utf8_lossy(&output.stdout);
    Ok(networks.lines().any(|line| line.contains(name)))
}

/// Remove configuration files for a port
fn remove_port_config(port_name: &str, eth_name: &str) -> Result<()> {
    let files_to_remove = vec![
        format!("/etc/systemd/network/{}.netdev", port_name),
        format!("/etc/systemd/network/{}.network", port_name),
        format!("/etc/systemd/network/{}.netdev", eth_name),
        format!("/etc/systemd/network/{}.network", eth_name),
    ];

    for file in files_to_remove {
        if std::path::Path::new(&file).exists() {
            fs::remove_file(&file).with_context(|| format!("removing config file {}", file))?;
            debug!("Removed config file: {}", file);
        }
    }

    Ok(())
}
