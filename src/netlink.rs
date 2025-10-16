use crate::fuse::{
    bind_veth_interface_enhanced, ensure_fuse_mount_base, unbind_veth_interface_enhanced,
};
use crate::interfaces::update_interfaces_block;
use crate::ledger::Ledger;
use crate::link;
use crate::naming::render_template;
use anyhow::{Context, Result};
use log::{info, warn};
use std::path::PathBuf;

/// Configuration for creating a container interface
#[derive(Debug, Clone)]
pub struct InterfaceConfig {
    pub bridge: String,
    pub raw_ifname: String,
    pub container_id: String,
    pub vmid: u32,
    pub interfaces_path: String,
    pub managed_tag: String,
    pub enable_rename: bool,
    pub naming_template: String,
    pub ledger_path: String,
}

impl InterfaceConfig {
    pub fn new(bridge: String, raw_ifname: String, container_id: String, vmid: u32) -> Self {
        Self {
            bridge,
            raw_ifname,
            container_id,
            vmid,
            interfaces_path: "/etc/network/interfaces".to_string(),
            managed_tag: "ovs-port-agent".to_string(),
            enable_rename: true,
            naming_template: "vi_{container}".to_string(),
            ledger_path: "/var/lib/ovs-port-agent/ledger.jsonl".to_string(),
        }
    }

    pub fn with_interfaces_path(mut self, path: String) -> Self {
        self.interfaces_path = path;
        self
    }

    pub fn with_managed_tag(mut self, tag: String) -> Self {
        self.managed_tag = tag;
        self
    }

    pub fn with_enable_rename(mut self, enable: bool) -> Self {
        self.enable_rename = enable;
        self
    }

    pub fn with_naming_template(mut self, template: String) -> Self {
        self.naming_template = template;
        self
    }

    pub fn with_ledger_path(mut self, path: String) -> Self {
        self.ledger_path = path;
        self
    }
}

/// Proactively create a container interface with proper vi{VMID} naming
/// This replaces the monitoring approach with immediate creation
pub async fn create_container_interface(config: InterfaceConfig) -> Result<()> {
    let interfaces_path = PathBuf::from(&config.interfaces_path);

    // Ensure FUSE mount base exists
    if let Err(err) = ensure_fuse_mount_base() {
        warn!("failed to ensure FUSE mount base: {err:?}");
    }

    // Generate the proper interface name using vi{VMID} format
    let target_name = if config.enable_rename {
        render_template(&config.naming_template, &config.container_id, 0)
    } else {
        config.raw_ifname.clone()
    };

    info!(
        "Creating container interface: {} -> {}",
        config.raw_ifname, target_name
    );

    // Rename the interface if needed
    #[allow(clippy::collapsible_if)]
    if config.enable_rename && config.raw_ifname != target_name {
        if !link::exists(&target_name) {
            if let Err(e) = link::rename_safely(&config.raw_ifname, &target_name) {
                warn!(
                    "rename {} -> {} failed: {:?}",
                    config.raw_ifname, target_name, e
                );
                return Err(e);
            } else {
                // Log the rename
                let mut lg = Ledger::open(PathBuf::from(&config.ledger_path))?;
                let _ = lg.append(
                    "interface_rename",
                    serde_json::json!({
                        "old": config.raw_ifname,
                        "new": target_name,
                        "bridge": config.bridge,
                        "container_id": config.container_id,
                        "vmid": config.vmid
                    }),
                );
            }
        }
    }

    // No NetworkManager connections needed - using OVS directly via D-Bus

    // Log the interface creation
    let mut lg = Ledger::open(PathBuf::from(&config.ledger_path))?;
    let _ = lg.append(
        "interface_created",
        serde_json::json!({
            "interface": target_name,
            "original": config.raw_ifname,
            "bridge": config.bridge,
            "container_id": config.container_id,
            "vmid": config.vmid
        }),
    );

    // Create enhanced FUSE bind mount for Proxmox visibility with full API compatibility
    if let Err(e) = bind_veth_interface_enhanced(
        &config.raw_ifname,
        &target_name,
        config.vmid,
        &config.container_id,
        &config.bridge,
    ) {
        warn!(
            "Failed to create enhanced Proxmox binding {} -> {}: {}",
            config.raw_ifname, target_name, e
        );
    }

    // Update /etc/network/interfaces for Proxmox GUI visibility
    let names = vec![target_name.clone()];
    update_interfaces_block(
        &interfaces_path,
        &config.managed_tag,
        &names,
        &config.bridge,
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

    // No NetworkManager connections to remove - using OVS directly via D-Bus

    // Remove enhanced FUSE bind mount with full cleanup
    if let Err(e) = unbind_veth_interface_enhanced(interface_name) {
        warn!(
            "Failed to unbind enhanced veth interface {}: {}",
            interface_name, e
        );
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
