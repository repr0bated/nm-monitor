use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

/// Return read-only OVS Bridge properties as JSON using ovs-vsctl --format=json
pub fn bridge_info_json(bridge: &str) -> Result<Value> {
    let out = Command::new("ovs-vsctl")
        .args([
            "--format=json",
            "--",
            "list",
            "Bridge",
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

