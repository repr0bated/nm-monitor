use anyhow::{Context, Result};
use std::process::Command;

pub fn exists(name: &str) -> bool {
    std::path::Path::new(&format!("/sys/class/net/{name}")).exists()
}

pub fn rename_safely(old: &str, new: &str) -> Result<()> {
    // down -> rename -> up
    let down = Command::new("ip").args(["link", "set", "dev", old, "down"]).status()
        .with_context(|| format!("ip link set dev {old} down"))?;
    if !down.success() {
        return Err(anyhow::anyhow!("failed to set {old} down"));
    }
    let rn = Command::new("ip").args(["link", "set", "dev", old, "name", new]).status()
        .with_context(|| format!("ip link set dev {old} name {new}"))?;
    if !rn.success() {
        // try to bring it up back with old name
        let _ = Command::new("ip").args(["link", "set", "dev", old, "up"]).status();
        return Err(anyhow::anyhow!("failed to rename {old} -> {new}"));
    }
    let up = Command::new("ip").args(["link", "set", "dev", new, "up"]).status()
        .with_context(|| format!("ip link set dev {new} up"))?;
    if !up.success() {
        return Err(anyhow::anyhow!("failed to set {new} up"));
    }
    Ok(())
}
