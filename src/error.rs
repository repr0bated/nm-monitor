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

    #[error("Blockchain error: {0}")]
    Blockchain(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("State management error: {0}")]
    State(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Container error: {0}")]
    Container(String),

    #[error("Systemd error: {0}")]
    Systemd(String),

    #[error("Netlink error: {0}")]
    Netlink(String),

    #[error("Btrfs error: {0}")]
    Btrfs(String),

    #[error("Ledger error: {0}")]
    Ledger(String),

    #[error("Metrics error: {0}")]
    Metrics(String),

    #[error("Internal error: {0}")]
    Internal(String),

    /// Catch-all for external errors that don't fit other categories
    #[error("External error: {0}")]
    External(anyhow::Error),
}

/// Result type alias for the OVS Port Agent
pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for zbus::fdo::Error {
    fn from(err: Error) -> Self {
        zbus::fdo::Error::Failed(err.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::External(err)
    }
}
