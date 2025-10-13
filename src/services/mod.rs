//! Service layer for business logic
//!
//! This module contains focused service implementations that handle
//! specific domains of functionality, keeping the D-Bus RPC layer thin.

pub mod blockchain;
pub mod bridge;
pub mod network_state;
pub mod port_management;

// Re-export commonly used types
pub use blockchain::BlockchainService;
pub use bridge::BridgeService;
pub use network_state::NetworkStateService;
pub use port_management::PortManagementService;
