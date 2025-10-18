//! Service layer for business logic
//!
//! This module contains focused service implementations that handle
//! specific domains of functionality, keeping the D-Bus RPC layer thin.

pub mod port_management;

// Re-export commonly used types
pub use port_management::PortManagementService;
