//! D-Bus RPC interface - thin layer that delegates to service implementations
//!
//! This module provides the D-Bus interface for the OVS Port Agent.
//! Business logic is delegated to focused service modules in `src/services/`.

use anyhow::{Context, Result};
use std::future;
use tracing::{debug, info};
use zbus::fdo::IntrospectableProxy;

use crate::fuse;
use crate::streaming_blockchain::StreamingBlockchain;
use crate::services::{
    BlockchainService, BridgeService, NetworkStateService, PortManagementService,
};
use crate::state::manager::StateManager;
use std::sync::Arc;

/// Application state shared across D-Bus methods
pub struct AppState {
    pub bridge: String,
    pub ledger_path: String,
    pub flow_manager: OvsFlowManager,
    pub state_manager: Option<Arc<StateManager>>,
    pub streaming_blockchain: Arc<StreamingBlockchain>,
}

/// D-Bus interface implementation
pub struct PortAgent {
    state: AppState,
    // Service instances
    port_service: PortManagementService,
    blockchain_service: BlockchainService,
    #[allow(dead_code)]
    bridge_service: BridgeService,
    network_service: NetworkStateService,
}

impl PortAgent {
    pub fn new(state: AppState) -> Self {
        let port_service = PortManagementService::new(&state.bridge, &state.ledger_path);
        let blockchain_service = BlockchainService::new(&state.ledger_path);
        let bridge_service = BridgeService::new(&state.bridge);
        let network_service = NetworkStateService::new();

        Self {
            state,
            port_service,
            blockchain_service,
            bridge_service,
            network_service,
        }
    }
}

#[zbus::interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    /// Ping the service to check if it's alive
    fn ping(&self) -> String {
        "pong".into()
    }

    // ========================================================================
    // Port Management Operations
    // ========================================================================

    /// List all ports on the configured bridge
    fn list_ports(&self) -> zbus::fdo::Result<Vec<String>> {
        debug!("D-Bus call: list_ports");
        self.port_service
            .list_ports()
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to list ports: {}", e)))
    }

    /// Add a port to the bridge
    fn add_port(&self, name: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: add_port({})", name);
        tokio::runtime::Handle::current()
            .block_on(async { self.port_service.add_port(name).await })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to add port '{}': {}", name, e)))
    }

    /// Remove a port from the bridge
    fn del_port(&self, name: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: del_port({})", name);
        tokio::runtime::Handle::current()
            .block_on(async { self.port_service.del_port(name).await })
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to delete port '{}': {}", name, e))
            })
    }

    // ========================================================================
    // Blockchain Ledger Operations
    // ========================================================================

    /// Get blockchain ledger statistics
    fn get_blockchain_stats(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: get_blockchain_stats");
        let stats = self.blockchain_service.get_stats().map_err(|e| {
            zbus::fdo::Error::Failed(format!("Failed to get blockchain stats: {}", e))
        })?;

        Ok(serde_json::to_string_pretty(&stats)
            .unwrap_or_else(|_| "Failed to serialize blockchain stats".to_string()))
    }

    /// Get blockchain blocks by category
    fn get_blocks_by_category(&self, category: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: get_blocks_by_category({})", category);
        let blocks = self
            .blockchain_service
            .get_blocks_by_category(category)
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!(
                    "Failed to get blocks for category '{}': {}",
                    category, e
                ))
            })?;

        Ok(serde_json::to_string_pretty(&blocks)
            .unwrap_or_else(|_| "Failed to serialize blocks".to_string()))
    }

    /// Get blockchain blocks by height range
    fn get_blocks_by_height(&self, start: u64, end: u64) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: get_blocks_by_height({}, {})", start, end);
        let blocks = self
            .blockchain_service
            .get_blocks_by_height(start, end)
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!(
                    "Failed to get blocks for height range {}-{}: {}",
                    start, end, e
                ))
            })?;

        Ok(serde_json::to_string_pretty(&blocks)
            .unwrap_or_else(|_| "Failed to serialize blocks".to_string()))
    }

    /// Verify blockchain integrity
    fn verify_blockchain_integrity(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: verify_blockchain_integrity");
        let is_valid = self
            .blockchain_service
            .verify_chain()
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to verify blockchain: {}", e)))?;

        Ok(if is_valid {
            "Blockchain integrity: VALID".to_string()
        } else {
            "Blockchain integrity: INVALID".to_string()
        })
    }

    /// Add data to blockchain through D-Bus
    fn add_blockchain_data(
        &self,
        category: &str,
        action: &str,
        data: &str,
    ) -> zbus::fdo::Result<String> {
        info!(
            "D-Bus call: add_blockchain_data({}, {}, ...)",
            category, action
        );
        let data_value = serde_json::from_str(data)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid JSON data: {}", e)))?;

        let block_hash = self
            .blockchain_service
            .add_data(category, action, data_value)
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to add blockchain data: {}", e))
            })?;

        Ok(format!(
            "Data added to blockchain with hash: {}",
            block_hash
        ))
    }

    /// Add event to streaming blockchain with vectorization
    fn add_blockchain_event(&self, category: &str, action: &str, data: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: add_blockchain_event({}, {}, ...)", category, action);
        
        let data_value: serde_json::Value = serde_json::from_str(data)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid JSON: {}", e)))?;

        let hash = tokio::runtime::Handle::current()
            .block_on(async { 
                self.state.streaming_blockchain.add_event(category, action, data_value).await 
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to add event: {}", e)))?;

        Ok(format!("Event added with hash: {}", hash))
    }

    /// Stream vectors to replica
    fn stream_vectors(&self, block_hash: &str, remote: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: stream_vectors({}, {})", block_hash, remote);
        
        tokio::runtime::Handle::current()
            .block_on(async { 
                self.state.streaming_blockchain.stream_vectors(block_hash, remote).await 
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to stream vectors: {}", e)))?;

        Ok(format!("Vectors streamed for block: {}", block_hash))
    }

    /// Stream to multiple replicas
    fn stream_to_replicas(&self, block_hash: &str, replicas: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: stream_to_replicas({}, ...)", block_hash);
        
        let replica_list: Vec<String> = serde_json::from_str(replicas)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid replica list: {}", e)))?;

        tokio::runtime::Handle::current()
            .block_on(async { 
                self.state.streaming_blockchain.stream_to_replicas(block_hash, &replica_list).await 
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to stream to replicas: {}", e)))?;

        Ok(format!("Streamed to {} replicas", replica_list.len()))
    }

    /// Query similar events by vector
    fn query_similar_events(&self, query_vector: &str, limit: u32) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: query_similar_events(..., {})", limit);
        
        let vector: Vec<f32> = serde_json::from_str(query_vector)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid vector: {}", e)))?;

        let events = tokio::runtime::Handle::current()
            .block_on(async { 
                self.state.streaming_blockchain.query_similar(&vector, limit as usize).await 
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to query events: {}", e)))?;

        Ok(serde_json::to_string_pretty(&events)
            .unwrap_or_else(|_| "Failed to serialize events".to_string()))
    }

    // ========================================================================
    // Bridge Operations
    // ========================================================================

    /// Get OVS bridge topology and state
    fn get_bridge_topology(&self, bridge: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: get_bridge_topology({})", bridge);
        let service = BridgeService::new(bridge);
        let topology = tokio::runtime::Handle::current()
            .block_on(async { service.get_topology().await })
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to get bridge topology: {}", e))
            })?;

        Ok(serde_json::to_string_pretty(&topology)
            .unwrap_or_else(|_| "Failed to serialize topology".to_string()))
    }

    /// Validate bridge connectivity and synchronization
    fn validate_bridge_connectivity(&self, bridge: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: validate_bridge_connectivity({})", bridge);
        let service = BridgeService::new(bridge);
        let validation = tokio::runtime::Handle::current()
            .block_on(async { service.validate_connectivity().await })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Bridge validation failed: {}", e)))?;

        Ok(serde_json::to_string_pretty(&validation)
            .unwrap_or_else(|_| "Failed to serialize validation results".to_string()))
    }

    /// Perform atomic bridge operation with enhanced safety
    fn atomic_bridge_operation(&self, bridge: &str, operation: &str) -> zbus::fdo::Result<String> {
        info!(
            "D-Bus call: atomic_bridge_operation({}, {})",
            bridge, operation
        );
        let service = BridgeService::new(bridge);
        tokio::runtime::Handle::current()
            .block_on(async { service.perform_atomic_operation(operation).await })
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Atomic operation '{}' failed: {}", operation, e))
            })
    }

    // ========================================================================
    // Network State Operations
    // ========================================================================

    /// Get comprehensive system network state
    fn get_system_network_state(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: get_system_network_state");
        let state = tokio::runtime::Handle::current()
            .block_on(async { self.network_service.get_comprehensive_state().await })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to get network state: {}", e)))?;

        Ok(serde_json::to_string_pretty(&state)
            .unwrap_or_else(|_| "Failed to serialize network state".to_string()))
    }

    /// Get interface bindings for Proxmox integration
    fn get_interface_bindings(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: get_interface_bindings");
        let bindings = fuse::get_interface_bindings()
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to get bindings: {}", e)))?;

        Ok(serde_json::to_string_pretty(&bindings)
            .unwrap_or_else(|_| "Failed to serialize bindings".to_string()))
    }

    /// Introspect systemd-networkd instead of NetworkManager
    fn introspect_systemd_networkd(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: introspect_systemd_networkd");
        tokio::runtime::Handle::current()
            .block_on(async { introspect_systemd_networkd().await })
            .map(|_| "systemd-networkd introspection completed successfully".to_string())
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("systemd-networkd introspection failed: {}", e))
            })
    }

    // ========================================================================
    // Declarative State Management
    // ========================================================================

    /// Apply declarative state from YAML/JSON
    fn apply_state(&self, state_yaml: &str) -> zbus::fdo::Result<String> {
        info!("D-Bus call: apply_state");

        let state_manager =
            self.state.state_manager.as_ref().ok_or_else(|| {
                zbus::fdo::Error::Failed("State manager not initialized".to_string())
            })?;

        // Parse the desired state
        let desired_state: crate::state::manager::DesiredState =
            serde_yaml::from_str(state_yaml)
                .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid YAML: {}", e)))?;

        // Apply the state
        let report = tokio::runtime::Handle::current()
            .block_on(async { state_manager.apply_state(desired_state).await })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to apply state: {}", e)))?;

        Ok(serde_json::to_string_pretty(&report)
            .unwrap_or_else(|_| "Failed to serialize apply report".to_string()))
    }

    /// Query current state across all plugins or a specific plugin
    fn query_state(&self, plugin: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: query_state({})", plugin);

        let state_manager =
            self.state.state_manager.as_ref().ok_or_else(|| {
                zbus::fdo::Error::Failed("State manager not initialized".to_string())
            })?;

        let state = tokio::runtime::Handle::current()
            .block_on(async {
                if plugin.is_empty() {
                    state_manager
                        .query_current_state()
                        .await
                        .map(|s| serde_json::to_value(&s).unwrap_or(serde_json::Value::Null))
                } else {
                    state_manager.query_plugin_state(plugin).await
                }
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to query state: {}", e)))?;

        Ok(serde_json::to_string_pretty(&state)
            .unwrap_or_else(|_| "Failed to serialize state".to_string()))
    }

    /// Show diff between current and desired state
    fn show_diff(&self, desired_yaml: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: show_diff");

        let state_manager =
            self.state.state_manager.as_ref().ok_or_else(|| {
                zbus::fdo::Error::Failed("State manager not initialized".to_string())
            })?;

        // Parse the desired state
        let desired_state: crate::state::manager::DesiredState = serde_yaml::from_str(desired_yaml)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid YAML: {}", e)))?;

        // Calculate diff
        let diffs = tokio::runtime::Handle::current()
            .block_on(async { state_manager.show_diff(desired_state).await })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to calculate diff: {}", e)))?;

        Ok(serde_json::to_string_pretty(&diffs)
            .unwrap_or_else(|_| "Failed to serialize diffs".to_string()))
    }

    // ========================================================================
    // OVS Flow Management
    // ========================================================================

    /// Clear all OVS flow rules
    fn clear_flows(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: clear_flows");
        self.state
            .flow_manager
            .clear_all_flows()
            .map(|_| "All OVS flows cleared successfully".to_string())
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to clear flows: {}", e)))
    }

    /// Setup container-specific routing
    fn setup_container_routing(
        &self,
        container_ip: &str,
        container_port: &str,
        register_id: u32,
    ) -> zbus::fdo::Result<String> {
        debug!(
            "D-Bus call: setup_container_routing({}, {}, {})",
            container_ip, container_port, register_id
        );
        self.state
            .flow_manager
            .setup_container_routing(container_ip, container_port, register_id)
            .map(|_| {
                format!(
                    "Container routing setup for {} via register {}",
                    container_ip, register_id
                )
            })
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to setup container routing: {}", e))
            })
    }

    /// Setup application-aware routing
    fn setup_application_routing(&self, output_port: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: setup_application_routing({})", output_port);
        self.state
            .flow_manager
            .setup_application_routing(output_port)
            .map(|_| "Application-aware routing setup successfully".to_string())
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Failed to setup application routing: {}", e))
            })
    }

    /// Setup basic container network routing
    fn setup_basic_routing(
        &self,
        container_network: &str,
        physical_interface: &str,
        internal_port: &str,
    ) -> zbus::fdo::Result<String> {
        debug!(
            "D-Bus call: setup_basic_routing({}, {}, {})",
            container_network, physical_interface, internal_port
        );
        self.state
            .flow_manager
            .setup_basic_routing(container_network, physical_interface, internal_port)
            .map(|_| {
                format!(
                    "Basic routing setup for container network {}",
                    container_network
                )
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to setup basic routing: {}", e)))
    }

    /// Dump current flow rules
    fn dump_flows(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: dump_flows");
        self.state
            .flow_manager
            .dump_flows()
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to dump flows: {}", e)))
    }
}

// ============================================================================
// D-Bus Service Functions
// ============================================================================

/// Serve the D-Bus interface with the given application state
pub async fn serve_with_state(state: AppState) -> Result<()> {
    let agent = PortAgent::new(state);
    let name = "dev.ovs.PortAgent1";
    let path = "/dev/ovs/PortAgent1";

    let _conn = zbus::connection::Builder::system()
        .context("Failed to connect to system bus")?
        .name(name)
        .with_context(|| format!("Failed to request D-Bus name '{}'", name))?
        .serve_at(path, agent)
        .with_context(|| format!("Failed to serve agent at path '{}'", path))?
        .build()
        .await
        .context("Failed to build D-Bus connection")?;

    info!("D-Bus service registered: {} at {}", name, path);
    future::pending::<()>().await;
    Ok(())
}

/// Introspect systemd-networkd for debugging
pub async fn introspect_systemd_networkd() -> Result<()> {
    info!("Performing comprehensive D-Bus introspection on systemd-networkd");
    let conn = zbus::Connection::system()
        .await
        .context("Failed to connect to system bus")?;

    introspect_object(
        &conn,
        "org.freedesktop.network1",
        "/org/freedesktop/network1",
    )
    .await
    .context("Failed to introspect systemd-networkd")?;

    Ok(())
}

/// Introspect a D-Bus object
async fn introspect_object(
    conn: &zbus::Connection,
    destination: &str,
    path: &str,
) -> Result<String> {
    let proxy = IntrospectableProxy::builder(conn)
        .destination(destination)
        .with_context(|| format!("Invalid D-Bus destination '{}'", destination))?
        .path(path)
        .with_context(|| format!("Invalid D-Bus path '{}'", path))?
        .build()
        .await
        .with_context(|| format!("Failed to create proxy for {}:{}", destination, path))?;

    proxy
        .introspect()
        .await
        .with_context(|| format!("Failed to introspect {}:{}", destination, path))
}
