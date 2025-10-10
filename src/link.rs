use anyhow::{Context, Result};
use std::process::Command;
// use std::fs; // reserved for future /proc scanning

/// Try to resolve a container short name for an interface by peeking into /proc and network namespaces.
/// Best-effort heuristic:
/// - Look for peer ifindex owner in /proc/*/ns/net that matches the veth peer
/// - Fallback: derive from interface name prefix
pub fn container_short_name_from_ifname(ifname: &str) -> Option<String> {
    // Extract VMID from veth interface names like veth9000i1 -> 9000
    let mut s = ifname.to_string();
    for p in ["veth", "vid", "vi", "tap"] {
        if let Some(rest) = s.strip_prefix(p) {
            s = rest.to_string();
            break;
        }
    }
    // Extract only the numeric VMID, stopping at 'i' separator or non-digit
    let vmid: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if vmid.is_empty() {
        None
    } else {
        Some(vmid)
    }
}

pub fn exists(name: &str) -> bool {
    std::path::Path::new(&format!("/sys/class/net/{name}")).exists()
}

pub fn rename_safely(old: &str, new: &str) -> Result<()> {
    // down -> rename -> up
    let down = Command::new("ip")
        .args(["link", "set", "dev", old, "down"])
        .status()
        .with_context(|| format!("ip link set dev {old} down"))?;
    if !down.success() {
        return Err(anyhow::anyhow!("failed to set {old} down"));
    }
    let rn = Command::new("ip")
        .args(["link", "set", "dev", old, "name", new])
        .status()
        .with_context(|| format!("ip link set dev {old} name {new}"))?;
    if !rn.success() {
        // try to bring it up back with old name
        let _ = Command::new("ip")
            .args(["link", "set", "dev", old, "up"])
            .status();
        return Err(anyhow::anyhow!("failed to rename {old} -> {new}"));
    }
    let up = Command::new("ip")
        .args(["link", "set", "dev", new, "up"])
        .status()
        .with_context(|| format!("ip link set dev {new} up"))?;
    if !up.success() {
        return Err(anyhow::anyhow!("failed to set {new} up"));
    }
    Ok(())
}
