use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn add_port(bridge: &str, port: &str) -> Result<()> {
    let status = Command::new("ovs-vsctl").args(["add-port", bridge, port]).status()
        .with_context(|| "spawning ovs-vsctl add-port")?;
    if !status.success() {
        bail!("ovs-vsctl add-port failed: status={:?}", status);
    }
    Ok(())
}

pub fn del_port(bridge: &str, port: &str) -> Result<()> {
    let status = Command::new("ovs-vsctl").args(["--if-exists", "del-port", bridge, port]).status()
        .with_context(|| "spawning ovs-vsctl del-port")?;
    if !status.success() {
        bail!("ovs-vsctl del-port failed: status={:?}", status);
    }
    Ok(())
}

pub fn list_ports(bridge: &str) -> Result<Vec<String>> {
    let output = Command::new("ovs-vsctl").args(["list-ports", bridge]).output()
        .with_context(|| "spawning ovs-vsctl list-ports")?;
    if !output.status.success() {
        bail!("ovs-vsctl list-ports failed: status={:?}", output.status);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ports: Vec<String> = stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(ports)
}
