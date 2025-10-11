use crate::fuse::{bind_veth_interface, ensure_fuse_mount_base, unbind_veth_interface};
use crate::interfaces::update_interfaces_block;
use crate::ledger::Ledger;
use crate::link;
use crate::naming::render_template;
use crate::nmcli_dyn;
use anyhow::{Context, Result};
use log::{info, warn};
use std::path::PathBuf;
/// Proactively create a container interface with proper vi{VMID} naming
/// This replaces the monitoring approach with immediate creation
#[allow(clippy::too_many_arguments)]
pub async fn create_container_interface(
    bridge: String,
    raw_ifname: &str,
    container_id: &str,
    vmid: u32,
    interfaces_path: String,
    managed_tag: String,
    enable_rename: bool,
    naming_template: String,
    ledger_path: String,
) -> Result<()> {
    let interfaces_path = PathBuf::from(interfaces_path);

    // Ensure FUSE mount base exists
    if let Err(err) = ensure_fuse_mount_base() {
        warn!("failed to ensure FUSE mount base: {err:?}");
    }

    // Generate the proper interface name using vi{VMID} format
    let target_name = if enable_rename {
        render_template(&naming_template, container_id, 0)
    } else {
        raw_ifname.to_string()
    };

    info!(
        "Creating container interface: {} -> {}",
        raw_ifname, target_name
    );

    // Rename the interface if needed
    #[allow(clippy::collapsible_if)]
    if enable_rename && raw_ifname != target_name {
        if !link::exists(&target_name) {
            if let Err(e) = link::rename_safely(raw_ifname, &target_name) {
                warn!("rename {raw_ifname} -> {target_name} failed: {e:?}");
                return Err(e);
            } else {
                // Log the rename
                let mut lg = Ledger::open(PathBuf::from(&ledger_path))?;
                let _ = lg.append(
                    "interface_rename",
                    serde_json::json!({
                        "old": raw_ifname,
                        "new": target_name,
                        "bridge": bridge,
                        "container_id": container_id,
                        "vmid": vmid
                    }),
                );
            }
        }
    }

    // Create NetworkManager connections for the interface
    nmcli_dyn::ensure_proactive_port(&bridge, &target_name)
        .with_context(|| format!("create NM connections for {target_name}"))?;

    // Log the interface creation
    let mut lg = Ledger::open(PathBuf::from(&ledger_path))?;
    let _ = lg.append(
        "interface_created",
        serde_json::json!({
            "interface": target_name,
            "original": raw_ifname,
            "bridge": bridge,
            "container_id": container_id,
            "vmid": vmid
        }),
    );

    // Create FUSE bind mount for Proxmox visibility
    // Source is the Proxmox veth identifier (pre-rename), destination is the OVS-facing interface
    if let Err(e) = bind_veth_interface(raw_ifname, &target_name) {
        warn!(
            "Failed to bind Proxmox veth {} to {}: {}",
            raw_ifname, target_name, e
        );
    }

    // Update /etc/network/interfaces for Proxmox GUI visibility
    let names = vec![target_name.clone()];
    update_interfaces_block(
        &interfaces_path,
        &managed_tag,
        &names,
        &bridge,
        None, // No uplink for container interfaces
    )?;

    info!("Successfully created container interface: {}", target_name);
    Ok(())
}

/// Remove a container interface and its associated resources
pub async fn remove_container_interface(
    bridge: String,
    interface_name: &str,
    interfaces_path: String,
    managed_tag: String,
    ledger_path: String,
) -> Result<()> {
    let interfaces_path = PathBuf::from(interfaces_path);

    info!("Removing container interface: {}", interface_name);

    // Remove NetworkManager connections using proactive naming
    let port_name = format!("ovs-port-{}", interface_name);
    let eth_name = format!("ovs-eth-{}", interface_name);

    info!("Removing OVS port connection: {}", port_name);
    nmcli_dyn::remove_proactive_port(&port_name, &eth_name)
        .with_context(|| format!("remove NM connections for {interface_name}"))?;

    // Remove FUSE bind mount
    if let Err(e) = unbind_veth_interface(interface_name) {
        warn!("Failed to unbind veth interface {}: {}", interface_name, e);
    }

    // Log the interface removal
    let mut lg = Ledger::open(PathBuf::from(&ledger_path))?;
    let _ = lg.append(
        "interface_removed",
        serde_json::json!({
            "interface": interface_name,
            "bridge": bridge
        }),
    );

    // Update /etc/network/interfaces
    let names: Vec<String> = Vec::new();
    update_interfaces_block(&interfaces_path, &managed_tag, &names, &bridge, None)?;

    info!(
        "Successfully removed container interface: {}",
        interface_name
    );
    Ok(())
}
