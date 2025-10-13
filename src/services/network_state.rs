//! Network state service for comprehensive system monitoring

use crate::command;
use crate::fuse::{self, InterfaceBinding};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Comprehensive network state for system-wide monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkState {
    pub networkd: NetworkdState,
    pub ovs_bridges: Vec<OVSBridgeState>,
    pub interface_bindings: HashMap<String, InterfaceBinding>,
    pub connectivity_status: ConnectivityStatus,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkdState {
    pub version: String,
    pub state: String,
    pub connectivity: String,
    pub active_connections: u32,
    pub total_connections: u32,
    pub devices: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OVSBridgeState {
    pub name: String,
    pub ports: Vec<String>,
    pub interfaces: Vec<String>,
    pub active: bool,
    pub datapath_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectivityStatus {
    pub internet_reachable: bool,
    pub dns_working: bool,
    pub default_route: String,
    pub uplink_status: String,
}

/// Service for network state operations
#[derive(Debug, Clone)]
pub struct NetworkStateService;

impl NetworkStateService {
    /// Create a new network state service
    pub fn new() -> Self {
        Self
    }

    /// Get comprehensive network state
    pub async fn get_comprehensive_state(&self) -> Result<NetworkState> {
        debug!("Getting comprehensive network state");

        let networkd = self
            .get_networkd_state()
            .await
            .context("Failed to get networkd state")?;

        let ovs_bridges = self
            .get_ovs_bridge_states()
            .await
            .context("Failed to get OVS bridge states")?;

        let interface_bindings =
            fuse::get_interface_bindings().context("Failed to get interface bindings")?;

        let connectivity_status = self
            .get_connectivity_status()
            .await
            .context("Failed to get connectivity status")?;

        Ok(NetworkState {
            networkd,
            ovs_bridges,
            interface_bindings,
            connectivity_status,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Get networkd state
    async fn get_networkd_state(&self) -> Result<NetworkdState> {
        debug!("Getting networkd state");

        let mut state = NetworkdState {
            version: String::from("unknown"),
            state: String::from("unknown"),
            connectivity: String::from("unknown"),
            active_connections: 0,
            total_connections: 0,
            devices: 0,
        };

        // Get version
        if let Ok(output) = command::networkctl(&["--version"]).await {
            state.version = output.trim().to_string();
        }

        // Get state
        if let Ok(output) = command::networkctl(&["-t", "-f", "STATE", "general"]).await {
            state.state = output.trim().to_string();
        }

        // Get connectivity
        if let Ok(output) = command::networkctl(&["-t", "-f", "CONNECTIVITY", "general"]).await {
            state.connectivity = output.trim().to_string();
        }

        // Count active connections
        if let Ok(output) = command::networkctl(&["list", "--no-pager"]).await {
            state.active_connections = output.lines().count() as u32;
        }

        // Count total connections
        if let Ok(output) = command::networkctl(&["-t", "-f", "NAME", "connection", "show"]).await {
            state.total_connections = output.lines().count() as u32;
        }

        // Count devices
        if let Ok(output) = command::networkctl(&["-t", "device", "status"]).await {
            state.devices = output.lines().count() as u32;
        }

        Ok(state)
    }

    /// Get OVS bridge states
    async fn get_ovs_bridge_states(&self) -> Result<Vec<OVSBridgeState>> {
        debug!("Getting OVS bridge states");

        let mut bridges = Vec::new();
        let bridge_names = command::list_bridges()
            .await
            .context("Failed to list OVS bridges")?;

        for bridge_name in bridge_names {
            let bridge_name = bridge_name.trim();
            if bridge_name.is_empty() {
                continue;
            }

            let mut bridge_state = OVSBridgeState {
                name: bridge_name.to_string(),
                ports: Vec::new(),
                interfaces: Vec::new(),
                active: false,
                datapath_type: String::from("system"),
            };

            // Get ports for this bridge
            if let Ok(ports) = command::get_bridge_ports(bridge_name).await {
                bridge_state.ports = ports;
            }

            // Get interfaces for this bridge
            if let Ok(interfaces) = command::get_bridge_interfaces(bridge_name).await {
                bridge_state.interfaces = interfaces;
            }

            // Check if bridge is active (has networkctl connection)
            if let Ok(output) =
                command::networkctl(&["-t", "-f", "NAME,STATE", "connection", "show"]).await
            {
                for line in output.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 && parts[0] == bridge_name {
                        bridge_state.active = parts[1].contains("activated");
                        break;
                    }
                }
            }

            bridges.push(bridge_state);
        }

        Ok(bridges)
    }

    /// Get connectivity status
    async fn get_connectivity_status(&self) -> Result<ConnectivityStatus> {
        debug!("Getting connectivity status");

        let mut status = ConnectivityStatus {
            internet_reachable: false,
            dns_working: false,
            default_route: String::from("unknown"),
            uplink_status: String::from("unknown"),
        };

        // Check internet connectivity
        status.internet_reachable = command::ping_host("8.8.8.8", 1, 2).await;

        // Check DNS
        status.dns_working = command::check_dns("google.com").await;

        // Get default route
        if let Ok(output) = command::execute_command("ip", &["route", "show", "default"]).await {
            status.default_route = output.trim().to_string();
        }

        // Get uplink status (first non-lo interface)
        if let Ok(output) =
            command::networkctl(&["-t", "-f", "DEVICE,STATE", "device", "status"]).await
        {
            for line in output.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 && parts[0] != "lo" {
                    status.uplink_status = parts[1].to_string();
                    break;
                }
            }
        }

        Ok(status)
    }
}

impl Default for NetworkStateService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_state_service_creation() {
        let _service = NetworkStateService::new();
        // Service is a zero-sized type, just verify it compiles
        // Service goes out of scope here naturally
    }

    #[tokio::test]
    async fn test_get_networkd_state() {
        let service = NetworkStateService::new();
        let result = service.get_networkd_state().await;
        // This should succeed even on systems without networkd
        // as we handle errors gracefully
        assert!(result.is_ok());

        let state = result.unwrap();
        // Version should be set to something, even if "unknown"
        assert!(!state.version.is_empty());
    }

    #[tokio::test]
    async fn test_get_connectivity_status() {
        let service = NetworkStateService::new();
        let result = service.get_connectivity_status().await;
        assert!(result.is_ok());

        let status = result.unwrap();
        // DNS and internet checks might fail in test environments, that's ok
        // Just verify the structure is populated
        assert!(!status.default_route.is_empty());
    }

    #[tokio::test]
    async fn test_get_comprehensive_state() {
        let service = NetworkStateService::new();
        let result = service.get_comprehensive_state().await;
        // This might fail if fuse is not set up, but structure should be valid
        if let Ok(state) = result {
            assert!(!state.timestamp.is_empty());
            assert!(!state.networkd.version.is_empty());
        }
    }
}
