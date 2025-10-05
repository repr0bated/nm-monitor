use anyhow::{Context, Result};
use std::fs;

/// Write NetworkManager unmanaged-devices configuration
/// This ensures NM only manages the bridge and its uplink, leaving everything else alone
pub fn write_unmanaged_devices(unmanaged: &[String]) -> Result<()> {
    if unmanaged.is_empty() {
        return Ok(());
    }

    let mut config = String::from("[keyfile]\n");
    config.push_str("unmanaged-devices=");

    let devices: Vec<String> = unmanaged
        .iter()
        .map(|iface| format!("interface-name:{}", iface))
        .collect();

    config.push_str(&devices.join(";"));
    config.push('\n');

    fs::write(
        "/etc/NetworkManager/conf.d/10-unmanaged-devices.conf",
        config,
    )
    .context("writing NM unmanaged-devices config")?;

    Ok(())
}
