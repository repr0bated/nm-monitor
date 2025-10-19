// Docker state plugin - manages Docker container introspection and state
// Handles: container discovery, filtering, and state management
use crate::plugin_footprint::PluginFootprint;
use crate::state::plugin::{
    ApplyResult, Checkpoint, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command as AsyncCommand;

/// Docker container configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    pub containers: Vec<ContainerConfig>,
    pub filters: Option<ContainerFilters>,
}

/// Container filtering options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerFilters {
    pub name_pattern: Option<String>,
    pub label_filters: Option<HashMap<String, String>>,
    pub status_filter: Option<String>,
    pub network_filter: Option<String>,
}

/// Individual container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub id: Option<String>,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: ContainerState,
    pub networks: Option<HashMap<String, NetworkConfig>>,
    pub ports: Option<Vec<PortConfig>>,
    pub labels: Option<HashMap<String, String>>,
    pub environment: Option<HashMap<String, String>>,
    pub volumes: Option<Vec<VolumeConfig>>,
    pub restart_policy: Option<String>,
    pub health_check: Option<HealthConfig>,

    /// Dynamic properties - introspection captures ALL container properties
    /// Examples: created_at, started_at, finished_at, exit_code, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,

    /// Property schema - tracks which fields exist (append-only set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_schema: Option<Vec<String>>,
}

/// Container state information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
}

/// Network configuration for containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
    pub mac_address: Option<String>,
}

/// Port configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    pub host_port: String,
    pub container_port: String,
    pub protocol: String,
}

/// Volume configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    pub host_path: Option<String>,
    pub container_path: String,
    pub mode: String,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    pub test: Vec<String>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
}

/// Docker state plugin implementation
pub struct DockerStatePlugin {
    #[allow(dead_code)]
    docker_socket: String,
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

impl DockerStatePlugin {
    pub fn new() -> Self {
        Self {
            docker_socket: "/var/run/docker.sock".to_string(),
            blockchain_sender: None,
        }
    }

    pub fn with_docker_socket(docker_socket: String) -> Self {
        Self {
            docker_socket,
            blockchain_sender: None,
        }
    }

    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            docker_socket: "/var/run/docker.sock".to_string(),
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Create footprint for Docker operations
    #[allow(dead_code)]
    fn create_footprint(&self, operation: &str, data: &Value) -> Result<()> {
        if let Some(sender) = &self.blockchain_sender {
            let mut metadata = HashMap::new();
            metadata.insert("plugin".to_string(), Value::String("docker".to_string()));
            metadata.insert("host".to_string(), Value::String(
                gethostname::gethostname().to_string_lossy().to_string()
            ));

            let footprint = crate::plugin_footprint::FootprintGenerator::new("docker")
                .create_footprint(operation, data, Some(metadata))?;

            sender.send(footprint)?;
        }
        Ok(())
    }

    /// Check if Docker daemon is available
    async fn check_docker_available(&self) -> Result<bool> {
        let output = AsyncCommand::new("docker")
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => Ok(true),
            _ => {
                log::info!("Docker not available - skipping Docker operations");
                Ok(false)
            }
        }
    }

    /// Query Docker containers using Docker API
    async fn query_docker_containers(&self) -> Result<Vec<ContainerConfig>> {
        let output = AsyncCommand::new("docker")
            .args([
                "ps",
                "-a",
                "--format",
                "table {{.ID}}\\t{{.Names}}\\t{{.Image}}\\t{{.Status}}\\t{{.Ports}}\\t{{.Labels}}\\t{{.Networks}}"
            ])
            .output()
            .await
            .context("Failed to execute docker ps")?;

        if !output.status.success() {
            return Err(anyhow!("Docker ps command failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut containers = Vec::new();

        // Skip header line
        for line in stdout.lines().skip(1) {
            if let Some(container) = self.parse_container_line(line).await {
                containers.push(container);
            }
        }

        Ok(containers)
    }

    /// Parse a single container line from docker ps output
    async fn parse_container_line(&self, line: &str) -> Option<ContainerConfig> {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 {
            return None;
        }

        let id = parts[0].to_string();
        let name = parts[1].trim_matches('"').to_string();
        let image = parts[2].to_string();
        let status = parts[3].to_string();
        let ports_str = parts[4].to_string();
        let labels_str = parts[5].to_string();
        let networks_str = parts[6].to_string();

        // Determine container state from status
        let state = self.parse_container_state(&status);

        // Parse ports
        let ports = if ports_str.trim().is_empty() {
            None
        } else {
            Some(self.parse_ports(&ports_str))
        };

        // Parse labels
        let labels = if labels_str.trim().is_empty() {
            None
        } else {
            Some(self.parse_labels(&labels_str))
        };

        // Parse networks
        let networks = if networks_str.trim().is_empty() {
            None
        } else {
            Some(self.parse_networks(&networks_str))
        };

        Some(ContainerConfig {
            id: Some(id),
            name,
            image,
            status,
            state,
            networks,
            ports,
            labels,
            environment: None,
            volumes: None,
            restart_policy: None,
            health_check: None,
            properties: None,
            property_schema: None,
        })
    }

    /// Parse container state from status string
    fn parse_container_state(&self, status: &str) -> ContainerState {
        if status.contains("Up") {
            ContainerState::Running
        } else if status.contains("Exited") {
            ContainerState::Exited
        } else if status.contains("Created") {
            ContainerState::Created
        } else if status.contains("Paused") {
            ContainerState::Paused
        } else if status.contains("Restarting") {
            ContainerState::Restarting
        } else if status.contains("Removing") {
            ContainerState::Removing
        } else if status.contains("Dead") {
            ContainerState::Dead
        } else {
            ContainerState::Created
        }
    }

    /// Parse port mappings
    fn parse_ports(&self, ports_str: &str) -> Vec<PortConfig> {
        let mut ports = Vec::new();

        // Simple parsing for port mappings like "0.0.0.0:51821->51821/udp, :::51822->51822/udp"
        for port_mapping in ports_str.split(", ") {
            if let Some((host_part, container_part)) = port_mapping.split_once("->") {
                if let Some((container_port, protocol)) = container_part.split_once("/") {
                    ports.push(PortConfig {
                        host_port: host_part.trim().to_string(),
                        container_port: container_port.trim().to_string(),
                        protocol: protocol.trim().to_string(),
                    });
                }
            }
        }

        ports
    }

    /// Parse container labels
    fn parse_labels(&self, labels_str: &str) -> HashMap<String, String> {
        let mut labels = HashMap::new();

        // Simple parsing for labels like "maintainer=gravitl, com.docker.compose.project=netmaker"
        for label in labels_str.split(", ") {
            if let Some((key, value)) = label.split_once("=") {
                labels.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        labels
    }

    /// Parse network information
    fn parse_networks(&self, networks_str: &str) -> HashMap<String, NetworkConfig> {
        let mut networks = HashMap::new();

        // Simple parsing for networks - in a real implementation,
        // we'd use docker inspect to get detailed network info
        if !networks_str.trim().is_empty() && networks_str != "bridge" {
            networks.insert(
                networks_str.trim().to_string(),
                NetworkConfig {
                    ip_address: None,
                    gateway: None,
                    mac_address: None,
                },
            );
        }

        networks
    }

    /// Filter containers based on configuration
    #[allow(dead_code)]
    fn filter_containers(&self, containers: Vec<ContainerConfig>, filters: &ContainerFilters) -> Vec<ContainerConfig> {
        containers
            .into_iter()
            .filter(|container| {
                // Filter by name pattern
                if let Some(pattern) = &filters.name_pattern {
                    if !container.name.contains(pattern) {
                        return false;
                    }
                }

                // Filter by status
                if let Some(status_filter) = &filters.status_filter {
                    if !container.status.to_lowercase().contains(&status_filter.to_lowercase()) {
                        return false;
                    }
                }

                // Filter by labels
                if let Some(label_filters) = &filters.label_filters {
                    if let Some(container_labels) = &container.labels {
                        for (key, value) in label_filters {
                            if let Some(container_value) = container_labels.get(key) {
                                if container_value != value {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                        }
                    } else {
                        return false;
                    }
                }

                // Filter by network
                if let Some(network_filter) = &filters.network_filter {
                    if let Some(networks) = &container.networks {
                        if !networks.contains_key(network_filter) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Get detailed container information using docker inspect
    #[allow(dead_code)]
    async fn get_container_details(&self, container_id: &str) -> Result<ContainerConfig> {
        let output = AsyncCommand::new("docker")
            .args(["inspect", container_id, "--format", "json"])
            .output()
            .await
            .context("Failed to inspect container")?;

        if !output.status.success() {
            return Err(anyhow!("Docker inspect failed for container {}", container_id));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the JSON output - this would be a single container in an array
        let containers: Vec<Value> = serde_json::from_str(&stdout)
            .context("Failed to parse docker inspect output")?;

        if containers.is_empty() {
            return Err(anyhow!("No container data returned for {}", container_id));
        }

        let container_data = &containers[0];

        // Extract detailed information
        let mut container = ContainerConfig {
            id: container_data.get("Id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            name: container_data
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim_matches('/')
                .to_string(),
            image: container_data
                .get("Config")
                .and_then(|c| c.get("Image"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            status: "unknown".to_string(), // We'll get this from state
            state: ContainerState::Created,
            networks: None,
            ports: None,
            labels: None,
            environment: None,
            volumes: None,
            restart_policy: None,
            health_check: None,
            properties: None,
            property_schema: None,
        };

        // Extract labels
        if let Some(labels) = container_data.get("Config").and_then(|c| c.get("Labels")) {
            let mut label_map = HashMap::new();
            if let Some(labels_obj) = labels.as_object() {
                for (key, value) in labels_obj {
                    if let Some(val_str) = value.as_str() {
                        label_map.insert(key.clone(), val_str.to_string());
                    }
                }
            }
            container.labels = Some(label_map);
        }

        // Extract environment variables
        if let Some(env) = container_data.get("Config").and_then(|c| c.get("Env")) {
            let mut env_map = HashMap::new();
            if let Some(env_array) = env.as_array() {
                for env_var in env_array {
                    if let Some(env_str) = env_var.as_str() {
                        if let Some((key, value)) = env_str.split_once('=') {
                            env_map.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }
            container.environment = Some(env_map);
        }

        // Extract volumes
        if let Some(volumes) = container_data.get("Mounts") {
            let mut volume_configs = Vec::new();
            if let Some(volumes_array) = volumes.as_array() {
                for volume in volumes_array {
                    if let (Some(source), Some(destination)) = (
                        volume.get("Source").and_then(|v| v.as_str()),
                        volume.get("Destination").and_then(|v| v.as_str()),
                    ) {
                        volume_configs.push(VolumeConfig {
                            host_path: Some(source.to_string()),
                            container_path: destination.to_string(),
                            mode: volume.get("Mode").and_then(|v| v.as_str()).unwrap_or("rw").to_string(),
                        });
                    }
                }
            }
            container.volumes = Some(volume_configs);
        }

        Ok(container)
    }
}

#[async_trait]
impl StatePlugin for DockerStatePlugin {
    fn name(&self) -> &str {
        "docker"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        if !self.check_docker_available().await? {
            return Ok(serde_json::json!({
                "containers": [],
                "error": "Docker daemon not available"
            }));
        }

        let containers = self.query_docker_containers().await?;

        Ok(serde_json::to_value(DockerConfig {
            containers,
            filters: None,
        })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: DockerConfig = serde_json::from_value(current.clone())?;
        let desired_config: DockerConfig = serde_json::from_value(desired.clone())?;

        let mut actions = Vec::new();

        // Build maps for quick lookup
        let current_map: HashMap<String, &ContainerConfig> = current_config
            .containers
            .iter()
            .filter_map(|c| c.id.as_ref().map(|id| (id.clone(), c)))
            .collect();

        let desired_map: HashMap<String, &ContainerConfig> = desired_config
            .containers
            .iter()
            .filter_map(|c| c.id.as_ref().map(|id| (id.clone(), c)))
            .collect();

        // Find containers to create or modify
        for (id, desired_container) in &desired_map {
            if let Some(current_container) = current_map.get(id) {
                // Check if modification needed
                if serde_json::to_value(current_container)? != serde_json::to_value(desired_container)? {
                    actions.push(StateAction::Modify {
                        resource: format!("container:{}", desired_container.name),
                        changes: serde_json::to_value(desired_container)?,
                    });
                }
            } else {
                actions.push(StateAction::Create {
                    resource: format!("container:{}", desired_container.name),
                    config: serde_json::to_value(desired_container)?,
                });
            }
        }

        // Find containers to delete
        for id in current_map.keys() {
            if !desired_map.contains_key(id) {
                if let Some(container) = current_map.get(id) {
                    actions.push(StateAction::Delete {
                        resource: format!("container:{}", container.name),
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
                StateAction::Create { resource, config }
                | StateAction::Modify {
                    resource,
                    changes: config,
                } => {
                    let _container_config: ContainerConfig = serde_json::from_value(config.clone())?;

                    // For Docker containers, we typically don't "apply" state changes
                    // as containers are managed externally. We just track their state.
                    changes_applied.push(format!("Tracked container state: {}", resource));
                }
                StateAction::Delete { resource } => {
                    // Container deletion is typically handled externally
                    changes_applied.push(format!("Noted container removal: {}", resource));
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
        let desired_config: DockerConfig = serde_json::from_value(desired.clone())?;
        let current = self.query_current_state().await?;
        let current_config: DockerConfig = serde_json::from_value(current)?;

        // Simple verification: check if desired containers exist and are in desired state
        let current_names: std::collections::HashSet<_> =
            current_config.containers.iter().map(|c| &c.name).collect();

        for container in &desired_config.containers {
            if !current_names.contains(&container.name) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("docker-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // For Docker containers, rollback is typically not applicable
        // as container state is managed externally
        log::info!("Docker plugin rollback is a no-op - container state managed externally");
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false, // Docker state is external
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // Container operations are external
        }
    }
}

impl Default for DockerStatePlugin {
    fn default() -> Self {
        Self::new()
    }
}
