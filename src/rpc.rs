//! D-Bus RPC interface - thin layer that delegates to service implementations

use anyhow::{Context, Result};
use std::future;
use tracing::{debug, info};

use crate::services::{
    BlockchainService, BridgeService, NetworkStateService, PortManagementService,
};
use crate::state::manager::StateManager;
use std::sync::Arc;

/// Application state shared across D-Bus methods
pub struct AppState {
    pub bridge: String,
    pub ledger_path: String,
    pub state_manager: Option<Arc<StateManager>>,
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
