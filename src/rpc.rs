//! D-Bus RPC interface - thin layer that delegates to service implementations

use anyhow::{Context, Result};
use std::future;
use std::sync::Arc;
use tracing::{debug, info};

use crate::services::{
    BlockchainService, BridgeService, NetworkStateService, PortManagementService,
};
use crate::state::manager::StateManager;
use crate::streaming_blockchain::StreamingBlockchain;
use crate::plugin_footprint::PluginFootprint;

/// Application state shared across D-Bus methods
pub struct AppState {
    pub bridge: String,
    pub ledger_path: String,
    pub state_manager: Option<Arc<StateManager>>,
    pub streaming_blockchain: Arc<StreamingBlockchain>,
    pub footprint_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
}

/// D-Bus interface implementation
pub struct PortAgent {
    state: AppState,
    port_service: PortManagementService,
    blockchain_service: BlockchainService,
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
    fn ping(&self) -> String {
        "pong".into()
    }

    fn list_ports(&self) -> zbus::fdo::Result<Vec<String>> {
        debug!("D-Bus call: list_ports");
        self.port_service
            .list_ports()
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to list ports: {}", e)))
    }

    async fn apply_state(&self, state_yaml: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: apply_state");
        if let Some(ref state_manager) = self.state.state_manager {
            match serde_yaml::from_str(state_yaml) {
                Ok(desired_state) => {
                    match state_manager.apply_state(desired_state).await {
                        Ok(report) => Ok(serde_json::to_string(&report).unwrap_or_default()),
                        Err(e) => Err(zbus::fdo::Error::Failed(format!("Apply state failed: {}", e)))
                    }
                }
                Err(e) => Err(zbus::fdo::Error::InvalidArgs(format!("Invalid YAML: {}", e)))
            }
        } else {
            Err(zbus::fdo::Error::Failed("State manager not available".into()))
        }
    }

    async fn query_state(&self, plugin: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: query_state for plugin: {}", plugin);
        if let Some(ref state_manager) = self.state.state_manager {
            match state_manager.query_plugin_state(plugin).await {
                Ok(state) => Ok(serde_json::to_string(&state).unwrap_or_default()),
                Err(e) => Err(zbus::fdo::Error::Failed(format!("Query failed: {}", e)))
            }
        } else {
            Err(zbus::fdo::Error::Failed("State manager not available".into()))
        }
    }

    async fn add_blockchain_event(&self, category: &str, action: &str, data: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: add_blockchain_event - {}/{}", category, action);
        match serde_json::from_str(data) {
            Ok(event_data) => {
                let footprint = crate::plugin_footprint::PluginFootprint::new(
                    category.to_string(),
                    action.to_string(),
                    event_data,
                );
                if let Err(e) = self.state.footprint_sender.send(footprint) {
                    Err(zbus::fdo::Error::Failed(format!("Failed to send footprint: {}", e)))
                } else {
                    Ok("Event added to blockchain".into())
                }
            }
            Err(e) => Err(zbus::fdo::Error::InvalidArgs(format!("Invalid JSON data: {}", e)))
        }
    }

    async fn stream_vectors(&self, block_hash: &str, remote: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: stream_vectors - {} to {}", block_hash, remote);
        match self.state.streaming_blockchain.stream_vectors(block_hash, remote).await {
            Ok(_) => Ok("Vectors streamed successfully".into()),
            Err(e) => Err(zbus::fdo::Error::Failed(format!("Stream failed: {}", e)))
        }
    }
}

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
