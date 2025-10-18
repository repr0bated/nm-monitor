//! nm-monitor library - streaming blockchain with plugin footprint mechanism
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod ovsdb_fuse;
pub mod btrfs_snapshot;
pub mod ovsdb_interceptor;
pub mod state;

pub mod plugin_footprint;
pub mod ovsdb_dbus;
pub mod zbus_networkd;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::plugins::{DockerStatePlugin, NetmakerStatePlugin};
    use crate::state::plugin::StatePlugin;

    #[tokio::test]
    async fn test_docker_plugin_creation() {
        let plugin = DockerStatePlugin::new();
        assert_eq!(plugin.name(), "docker");
        assert_eq!(plugin.version(), "1.0.0");
    }

    #[tokio::test]
    async fn test_netmaker_plugin_creation() {
        let plugin = NetmakerStatePlugin::new();
        assert_eq!(plugin.name(), "netmaker");
        assert_eq!(plugin.version(), "1.0.0");
    }

    #[tokio::test]
    async fn test_docker_plugin_query() {
        let plugin = DockerStatePlugin::new();

        // This might fail if Docker is not available, but should not panic
        let result = plugin.query_current_state().await;

        // If Docker is available, we should get a valid result
        if let Ok(state) = result {
            println!("Docker state: {}", serde_json::to_string_pretty(&state).unwrap_or_default());
        }
        // If Docker is not available, we expect an error but the plugin should handle it gracefully
    }
}
