use anyhow::{Context, Result};
use std::process::Command;
use std::fs;

/// Try to resolve a container short name for an interface by peeking into /proc and network namespaces.
/// Best-effort heuristic:
/// - Look for peer ifindex owner in /proc/*/ns/net that matches the veth peer
/// - Fallback: derive from interface name prefix
pub fn container_short_name_from_ifname(ifname: &str) -> Option<String> {
    // Placeholder heuristic: strip common prefixes and trailing digits
    let mut s = ifname.to_string();
    for p in ["veth-", "veth", "tap-"] { if let Some(rest) = s.strip_prefix(p) { s = rest.to_string(); break; } }
    let cleaned: String = s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c=='-').collect();
    Some(cleaned)
}

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
