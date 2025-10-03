use anyhow::{anyhow, bail, Context, Result};
use std::process::Command;

pub fn add_port(bridge: &str, port: &str) -> Result<()> {
    let output = Command::new("ovs-vsctl").args(["--may-exist", "add-port", bridge, port]).output()
        .with_context(|| "spawning ovs-vsctl add-port")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ovs-vsctl add-port failed: status={:?}, err={} ", output.status, stderr.trim());
    }
    Ok(())
}

pub fn del_port(bridge: &str, port: &str) -> Result<()> {
    let output = Command::new("ovs-vsctl").args(["--if-exists", "del-port", bridge, port]).output()
        .with_context(|| "spawning ovs-vsctl del-port")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ovs-vsctl del-port failed: status={:?}, err={}", output.status, stderr.trim());
    }
    Ok(())
}

pub fn list_ports(bridge: &str) -> Result<Vec<String>> {
    let output = Command::new("ovs-vsctl").args(["list-ports", bridge]).output()
        .with_context(|| "spawning ovs-vsctl list-ports")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ovs-vsctl list-ports failed: status={:?}, err={}", output.status, stderr.trim());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ports: Vec<String> = stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(ports)
}

pub fn ensure_bridge(bridge: &str) -> Result<()> {
    // Check if bridge exists
    let status = Command::new("ovs-vsctl").args(["br-exists", bridge]).status()
        .with_context(|| "spawning ovs-vsctl br-exists")?;
    if status.success() {
        return Ok(());
    }
    // Create if missing (idempotent)
    let output = Command::new("ovs-vsctl").args(["--may-exist", "add-br", bridge]).output()
        .with_context(|| "spawning ovs-vsctl add-br")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("failed to ensure bridge {}: {}", bridge, stderr.trim()));
    }
    Ok(())
}
