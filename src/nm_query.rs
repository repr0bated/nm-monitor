use anyhow::Result;

/// List connection names (network units for systemd-networkd)
pub fn list_connection_names() -> Result<Vec<String>> {
    // For now, use a synchronous approach with tokio runtime
    // In the future, this could be made async throughout the codebase
    tokio::runtime::Handle::current()
        .block_on(async { crate::systemd_dbus::list_network_units().await })
}
