//! Comprehensive error handling for the OVS Port Agent

use thiserror::Error;

/// Main error type for the OVS Port Agent
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("NetworkManager error: {0}")]
    NetworkManager(String),

    #[error("OVS error: {0}")]
    Ovs(String),

    #[error("D-Bus error: {0}")]
    Dbus(#[from] zbus::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Bridge error: {0}")]
    Bridge(String),

    #[error("Port error: {0}")]
    Port(String),

    #[error("FUSE error: {0}")]
    Fuse(String),

    #[error("Ledger error: {0}")]
    Ledger(String),

    #[error("Metrics error: {0}")]
    Metrics(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for the OVS Port Agent
pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for zbus::fdo::Error {
    fn from(err: Error) -> Self {
        zbus::fdo::Error::Failed(err.to_string())
    }
}
