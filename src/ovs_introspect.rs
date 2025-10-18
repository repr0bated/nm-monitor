use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

/// Return read-only OVS entity properties as JSON using ovs-vsctl --format=json
pub fn ovs_entity_info_json(entity_type: &str, name: &str) -> Result<Value> {
    let out = Command::new("ovs-vsctl")
        .args([
            "--format=json",
            "--",
            "list",
            entity_type,
            name,
        ])
        .output()
        .context("failed to execute ovs-vsctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "ovs-vsctl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("parse ovs-vsctl json")?;
    Ok(v)
}

/// Return read-only OVS Bridge properties as JSON using ovs-vsctl --format=json
pub fn bridge_info_json(bridge: &str) -> Result<Value> {
    ovs_entity_info_json("Bridge", bridge)
}

/// Return all bridges as JSON array
pub fn list_bridges_json() -> Result<Value> {
    let out = Command::new("ovs-vsctl")
        .args([
            "--format=json",
            "--",
            "list-br",
        ])
        .output()
        .context("failed to execute ovs-vsctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "ovs-vsctl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("parse ovs-vsctl json")?;
    Ok(v)
}

/// Return all ports on a bridge as JSON
pub fn bridge_ports_json(bridge: &str) -> Result<Value> {
    let out = Command::new("ovs-vsctl")
        .args([
            "--format=json",
            "--",
            "list-ports",
            bridge,
        ])
        .output()
        .context("failed to execute ovs-vsctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "ovs-vsctl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("parse ovs-vsctl json")?;
    Ok(v)
}

/// Return port info as JSON
pub fn port_info_json(port: &str) -> Result<Value> {
    ovs_entity_info_json("Port", port)
}

/// Return interface info as JSON
pub fn interface_info_json(iface: &str) -> Result<Value> {
    ovs_entity_info_json("Interface", iface)
}

/// Return all interfaces as JSON array
pub fn list_interfaces_json() -> Result<Value> {
    let out = Command::new("ovs-vsctl")
        .args([
            "--format=json",
            "--",
            "list",
            "Interface",
        ])
        .output()
        .context("failed to execute ovs-vsctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "ovs-vsctl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("parse ovs-vsctl json")?;
    Ok(v)
}

