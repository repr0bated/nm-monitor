// Netmaker state plugin - specialized Docker plugin for Netmaker container introspection
// Handles: Netmaker-specific container filtering, WireGuard network introspection, and state management
use crate::plugin_footprint::PluginFootprint;
use crate::state::plugin::{
    ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use crate::state::plugins::docker::{ContainerConfig, DockerConfig, ContainerFilters};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command as AsyncCommand;

/// Netmaker-specific configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerConfig {
    pub containers: Vec<NetmakerContainerConfig>,
    pub filters: Option<NetmakerFilters>,
    pub network_analysis: Option<NetworkAnalysis>,
}

/// Netmaker-specific filtering options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerFilters {
    pub name_pattern: Option<String>,
    pub netmaker_role: Option<String>, // "server", "client", "ingress", "egress"
    pub network_name: Option<String>,
    pub node_id: Option<String>,
    pub public_key_filter: Option<String>,
}

/// Enhanced container configuration for Netmaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerContainerConfig {
    #[serde(flatten)]
    pub base: ContainerConfig,

    /// Netmaker-specific fields
    pub netmaker_role: Option<String>,
    pub netmaker_network: Option<String>,
    pub node_id: Option<String>,
    pub public_key: Option<String>,
    pub private_key_path: Option<String>,
    pub interface_name: Option<String>,

    /// WireGuard network introspection data
    pub wireguard_peers: Option<Vec<WireGuardPeer>>,
    pub wireguard_interface: Option<WireGuardInterface>,
    pub netmaker_api_config: Option<NetmakerAPIConfig>,

    /// Network connectivity data
    pub connectivity_matrix: Option<HashMap<String, ConnectivityStatus>>,
}

/// WireGuard peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardPeer {
    pub public_key: String,
    pub allowed_ips: Vec<String>,
    pub endpoint: Option<String>,
    pub persistent_keepalive: Option<u32>,
    pub last_handshake: Option<String>,
}

/// WireGuard interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardInterface {
    pub name: String,
    pub public_key: String,
    pub private_key: Option<String>,
    pub listen_port: u16,
    pub fwmark: Option<u32>,
}

/// Netmaker API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetmakerAPIConfig {
    pub api_host: String,
    pub api_port: u16,
    pub base_url: String,
    pub auth_token: Option<String>,
}

/// Network analysis data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAnalysis {
    pub total_nodes: u32,
    pub connected_nodes: u32,
    pub network_topology: HashMap<String, Vec<String>>,
    pub connectivity_health: HashMap<String, f64>,
}

/// Connectivity status for each peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityStatus {
    pub status: String, // "connected", "disconnected", "degraded"
    pub latency_ms: Option<u32>,
    pub last_seen: Option<String>,
    pub packet_loss: Option<f64>,
}

/// Netmaker state plugin implementation
pub struct NetmakerStatePlugin {
    docker_plugin: crate::state::plugins::DockerStatePlugin,
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

impl NetmakerStatePlugin {
    pub fn new() -> Self {
        Self {
            docker_plugin: crate::state::plugins::DockerStatePlugin::new(),
            blockchain_sender: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            docker_plugin: crate::state::plugins::DockerStatePlugin::new(),
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Create footprint for Netmaker operations
    #[allow(dead_code)]
    fn create_footprint(&self, operation: &str, data: &Value) -> Result<()> {
        if let Some(sender) = &self.blockchain_sender {
            let mut metadata = HashMap::new();
            metadata.insert("plugin".to_string(), Value::String("netmaker".to_string()));
            metadata.insert("host".to_string(), Value::String(
                gethostname::gethostname().to_string_lossy().to_string()
            ));

            let footprint = crate::plugin_footprint::FootprintGenerator::new("netmaker")
                .create_footprint(operation, data, Some(metadata))?;

            sender.send(footprint)?;
        }
        Ok(())
    }

    /// Filter containers for Netmaker-specific ones
    async fn filter_netmaker_containers(&self, containers: Vec<ContainerConfig>) -> Vec<NetmakerContainerConfig> {
        let mut netmaker_containers = Vec::new();

        for container in containers {
            // Check if this is a Netmaker container by looking for specific labels or image patterns
            if self.is_netmaker_container(&container).await {
                if let Ok(netmaker_container) = self.enrich_netmaker_container(container).await {
                    netmaker_containers.push(netmaker_container);
                }
            }
        }

        netmaker_containers
    }

    /// Check if a container is a Netmaker container
    async fn is_netmaker_container(&self, container: &ContainerConfig) -> bool {
        // Check for Netmaker-specific labels
        if let Some(labels) = &container.labels {
            if labels.contains_key("netmaker.role") ||
               labels.contains_key("com.docker.compose.service") && (
                   container.name.contains("netmaker") ||
                   labels.values().any(|v| v.contains("netmaker"))
               ) {
                return true;
            }
        }

        // Check image name for Netmaker patterns
        if container.image.contains("netmaker") ||
           container.image.contains("gravitl") {
            return true;
        }

        // Check container name patterns
        if container.name.contains("netmaker") ||
           container.name.contains("nm-") ||
           container.name.starts_with("netmaker-") {
            return true;
        }

        false
    }

    /// Enrich container with Netmaker-specific information
    async fn enrich_netmaker_container(&self, container: ContainerConfig) -> Result<NetmakerContainerConfig> {
        // Determine Netmaker role from labels or container name
        let netmaker_role = self.determine_netmaker_role(&container).await;

        // Extract network information
        let netmaker_network = self.extract_netmaker_network(&container).await;

        // Get WireGuard interface information if this is a Netmaker node
        let wireguard_interface = if netmaker_role.is_some() {
            self.get_wireguard_interface(&container).await.ok()
        } else {
            None
        };

        // Get WireGuard peers if this is a Netmaker node
        let wireguard_peers = if netmaker_role.is_some() {
            self.get_wireguard_peers(&container).await.ok()
        } else {
            None
        };

        // Extract node ID from labels or environment
        let node_id = self.extract_node_id(&container).await;

        Ok(NetmakerContainerConfig {
            base: container.clone(),
            netmaker_role,
            netmaker_network,
            node_id,
            public_key: wireguard_interface.as_ref().map(|wg| wg.public_key.clone()),
            private_key_path: None, // Would need to inspect container filesystem
            interface_name: wireguard_interface.as_ref().map(|wg| wg.name.clone()),
            wireguard_peers,
            wireguard_interface,
            netmaker_api_config: self.extract_api_config(&container).await.ok(),
            connectivity_matrix: None, // Would require network probing
        })
    }

    /// Determine Netmaker role from container metadata
    async fn determine_netmaker_role(&self, container: &ContainerConfig) -> Option<String> {
        // Check labels first
        if let Some(labels) = &container.labels {
            if let Some(role) = labels.get("netmaker.role") {
                return Some(role.clone());
            }

            // Check for compose service names that indicate roles
            if let Some(service) = labels.get("com.docker.compose.service") {
                return match service.as_str() {
                    "netmaker" | "netmaker-server" => Some("server".to_string()),
                    "netmaker-ui" => Some("ui".to_string()),
                    name if name.contains("client") => Some("client".to_string()),
                    name if name.contains("ingress") => Some("ingress".to_string()),
                    name if name.contains("egress") => Some("egress".to_string()),
                    _ => None,
                };
            }
        }

        // Fallback to container name patterns
        if container.name.contains("server") || container.name.contains("netmaker") {
            Some("server".to_string())
        } else if container.name.contains("client") {
            Some("client".to_string())
        } else {
            None
        }
    }

    /// Extract Netmaker network name from container metadata
    async fn extract_netmaker_network(&self, container: &ContainerConfig) -> Option<String> {
        if let Some(labels) = &container.labels {
            if let Some(network) = labels.get("netmaker.network") {
                return Some(network.clone());
            }
        }

        // Check environment variables
        if let Some(env) = &container.environment {
            if let Some(network) = env.get("NETMAKER_NETWORK") {
                return Some(network.clone());
            }
        }

        None
    }

    /// Extract node ID from container metadata
    async fn extract_node_id(&self, container: &ContainerConfig) -> Option<String> {
        if let Some(labels) = &container.labels {
            if let Some(node_id) = labels.get("netmaker.node_id") {
                return Some(node_id.clone());
            }
        }

        // Check environment variables
        if let Some(env) = &container.environment {
            if let Some(node_id) = env.get("NODE_ID") {
                return Some(node_id.clone());
            }
        }

        None
    }

    /// Get WireGuard interface information from container
    async fn get_wireguard_interface(&self, _container: &ContainerConfig) -> Result<WireGuardInterface> {
        // This would require inspecting the container's WireGuard configuration
        // For now, we'll return a placeholder
        Ok(WireGuardInterface {
            name: "netmaker".to_string(),
            public_key: "placeholder_public_key".to_string(),
            private_key: None,
            listen_port: 51821,
            fwmark: None,
        })
    }

    /// Get WireGuard peers from container
    async fn get_wireguard_peers(&self, _container: &ContainerConfig) -> Result<Vec<WireGuardPeer>> {
        // This would require inspecting the container's WireGuard peer configuration
        // For now, we'll return a placeholder
        Ok(vec![])
    }

    /// Extract Netmaker API configuration
    async fn extract_api_config(&self, _container: &ContainerConfig) -> Result<NetmakerAPIConfig> {
        // This would require inspecting the container's API configuration
        // For now, we'll return a placeholder
        Ok(NetmakerAPIConfig {
            api_host: "localhost".to_string(),
            api_port: 8081,
            base_url: "/api".to_string(),
            auth_token: None,
        })
    }

    /// Perform network connectivity analysis
    async fn analyze_network_connectivity(&self, containers: &[NetmakerContainerConfig]) -> Result<NetworkAnalysis> {
        let mut total_nodes = 0u32;
        let mut connected_nodes = 0u32;
        let mut topology = HashMap::new();
        let mut health = HashMap::new();

        for container in containers {
            if let Some(_role) = &container.netmaker_role {
                total_nodes += 1;

                // Check if container is running and healthy
                if container.base.state == crate::state::plugins::docker::ContainerState::Running {
                    connected_nodes += 1;
                }

                // Build topology based on network connections
                if let Some(network) = &container.netmaker_network {
                    topology.entry(network.clone()).or_insert_with(Vec::new);
                }

                // Calculate health score (simplified)
                let health_score = if container.base.state == crate::state::plugins::docker::ContainerState::Running {
                    1.0
                } else {
                    0.5
                };

                health.insert(container.base.name.clone(), health_score);
            }
        }

        Ok(NetworkAnalysis {
            total_nodes,
            connected_nodes,
            network_topology: topology,
            connectivity_health: health,
        })
    }
}

#[async_trait]
impl StatePlugin for NetmakerStatePlugin {
    fn name(&self) -> &str {
        "netmaker"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        // First get all Docker containers
        let docker_state = self.docker_plugin.query_current_state().await?;
        let docker_config: DockerConfig = serde_json::from_value(docker_state)?;

        // Filter for Netmaker containers and enrich them
        let netmaker_containers = self.filter_netmaker_containers(docker_config.containers).await;

        // Perform network analysis
        let network_analysis = self.analyze_network_connectivity(&netmaker_containers).await.ok();

        Ok(serde_json::to_value(NetmakerConfig {
            containers: netmaker_containers,
            filters: None,
            network_analysis,
        })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: NetmakerConfig = serde_json::from_value(current.clone())?;
        let desired_config: NetmakerConfig = serde_json::from_value(desired.clone())?;

        let mut actions = Vec::new();

        // Build maps for quick lookup by container name
        let current_map: HashMap<String, &NetmakerContainerConfig> = current_config
            .containers
            .iter()
            .map(|c| (c.base.name.clone(), c))
            .collect();

        let desired_map: HashMap<String, &NetmakerContainerConfig> = desired_config
            .containers
            .iter()
            .map(|c| (c.base.name.clone(), c))
            .collect();

        // Find containers to create or modify
        for (name, desired_container) in &desired_map {
            if let Some(current_container) = current_map.get(name) {
                // Check if modification needed
                if serde_json::to_value(current_container)? != serde_json::to_value(desired_container)? {
                    actions.push(StateAction::Modify {
                        resource: format!("netmaker_container:{}", name),
                        changes: serde_json::to_value(desired_container)?,
                    });
                }
            } else {
                actions.push(StateAction::Create {
                    resource: format!("netmaker_container:{}", name),
                    config: serde_json::to_value(desired_container)?,
                });
            }
        }

        // Find containers to delete
        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                if let Some(_container) = current_map.get(name) {
                    actions.push(StateAction::Delete {
                        resource: format!("netmaker_container:{}", name),
                    });
                }
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: crate::state::plugin::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(serde_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(serde_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config: _ }
                | StateAction::Modify {
                    resource,
                    changes: _,
                } => {
                    // For Netmaker containers, we typically don't "apply" state changes
                    // as they are managed by Netmaker itself. We just track their state.
                    changes_applied.push(format!("Tracked Netmaker container state: {}", resource));
                }
                StateAction::Delete { resource } => {
                    // Container deletion is handled externally by Netmaker/Docker
                    changes_applied.push(format!("Noted Netmaker container removal: {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let desired_config: NetmakerConfig = serde_json::from_value(desired.clone())?;
        let current = self.query_current_state().await?;
        let current_config: NetmakerConfig = serde_json::from_value(current)?;

        // Check if desired containers exist and are in desired state
        let current_names: std::collections::HashSet<_> =
            current_config.containers.iter().map(|c| &c.base.name).collect();

        for container in &desired_config.containers {
            if !current_names.contains(&container.base.name) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("netmaker-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // For Netmaker containers, rollback is typically not applicable
        // as container state is managed externally
        log::info!("Netmaker plugin rollback is a no-op - container state managed externally");
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false, // Netmaker state is external
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // Container operations are external
        }
    }
}

impl Default for NetmakerStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}
