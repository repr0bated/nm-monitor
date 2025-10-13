//! Configuration management with validation

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use validator::Validate;
use crate::error::{Error, Result};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Config {
    /// Bridge configuration
    #[validate(nested)]
    pub bridge: BridgeConfig,

    /// NetworkManager configuration
    #[validate(nested)]
    pub network_manager: NetworkManagerConfig,

    /// FUSE configuration for Proxmox integration
    #[validate(nested)]
    pub fuse: FuseConfig,

    /// Ledger configuration
    #[validate(nested)]
    pub ledger: LedgerConfig,

    /// Metrics configuration
    #[validate(nested)]
    pub metrics: MetricsConfig,

    /// Logging configuration
    #[validate(nested)]
    pub logging: LoggingConfig,
}

/// Bridge-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BridgeConfig {
    /// Name of the OVS bridge to manage
    #[validate(length(min = 1, max = 15))]
    pub name: String,

    /// Physical uplink interface (optional)
    #[validate(length(max = 15))]
    pub uplink: Option<String>,

    /// Bridge datapath type
    #[validate(length(max = 20))]
    pub datapath_type: Option<String>,

    /// Fail mode for bridge
    #[validate(length(max = 10))]
    pub fail_mode: Option<String>,

    /// Enable STP
    pub stp_enable: bool,

    /// Enable RSTP
    pub rstp_enable: bool,

    /// Enable multicast snooping
    pub mcast_snooping_enable: bool,
}

/// NetworkManager configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct NetworkManagerConfig {
    /// Path to interfaces file for Proxmox compatibility
    #[validate(length(min = 1))]
    pub interfaces_path: String,

    /// Prefixes for interfaces to include as container ports
    #[validate(length(min = 1))]
    pub include_prefixes: Vec<String>,

    /// Tag for managed blocks in interfaces file
    #[validate(length(min = 1, max = 50))]
    pub managed_block_tag: String,

    /// Naming template for interfaces
    #[validate(length(min = 1, max = 50))]
    pub naming_template: String,

    /// Enable interface renaming
    pub enable_rename: bool,

    /// Interfaces that NetworkManager should NOT manage
    #[serde(default)]
    pub unmanaged_devices: Vec<String>,

    /// Connection timeout in seconds
    #[validate(range(min = 1, max = 300))]
    pub connection_timeout: u32,
}

/// FUSE configuration for Proxmox integration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FuseConfig {
    /// Enable FUSE integration
    pub enabled: bool,

    /// Base mount point for FUSE filesystem
    #[validate(length(min = 1))]
    pub mount_base: String,

    /// Proxmox API compatibility path
    #[validate(length(min = 1))]
    pub proxmox_api_base: String,
}

/// Ledger configuration for blockchain-style audit logging
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LedgerConfig {
    /// Enable blockchain ledger
    pub enabled: bool,

    /// Path to ledger file
    #[validate(length(min = 1))]
    pub path: String,

    /// Maximum ledger size in MB
    #[validate(range(min = 1, max = 10000))]
    pub max_size_mb: u32,

    /// Compression enabled
    pub compression_enabled: bool,
}

/// Metrics configuration for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Metrics server port
    #[validate(range(min = 1024, max = 65535))]
    pub port: u16,

    /// Metrics endpoint path
    #[validate(length(min = 1))]
    pub path: String,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LoggingConfig {
    /// Log level
    #[validate(length(min = 1))]
    pub level: String,

    /// Enable structured logging
    pub structured: bool,

    /// Enable journald integration
    pub journald: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bridge: BridgeConfig::default(),
            network_manager: NetworkManagerConfig::default(),
            fuse: FuseConfig::default(),
            ledger: LedgerConfig::default(),
            metrics: MetricsConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            name: "ovsbr0".to_string(),
            uplink: None,
            datapath_type: None,
            fail_mode: None,
            stp_enable: false,
            rstp_enable: false,
            mcast_snooping_enable: true,
        }
    }
}

impl Default for NetworkManagerConfig {
    fn default() -> Self {
        Self {
            interfaces_path: "/etc/network/interfaces".to_string(),
            include_prefixes: vec!["veth".to_string(), "tap".to_string()],
            managed_block_tag: "ovs-port-agent".to_string(),
            naming_template: "vi_{container}".to_string(),
            enable_rename: false,
            unmanaged_devices: vec![],
            connection_timeout: 45,
        }
    }
}

impl Default for FuseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mount_base: "/var/lib/ovs-port-agent/fuse".to_string(),
            proxmox_api_base: "/var/lib/ovs-port-agent/proxmox".to_string(),
        }
    }
}

impl Default for LedgerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/var/lib/ovs-port-agent/ledger.jsonl".to_string(),
            max_size_mb: 100,
            compression_enabled: true,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 9090,
            path: "/metrics".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            structured: true,
            journald: true,
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(path: Option<&Path>) -> Result<Self> {
        // Try explicit path, then /etc, then project-local config/example
        let candidates = if let Some(p) = path {
            vec![p.to_path_buf()]
        } else {
            vec![
                "/etc/ovs-port-agent/config.toml".into(),
                "config/config.toml".into(),
                "config/config.toml.example".into(),
            ]
        };

        for candidate in candidates {
            if candidate.exists() {
                let data = fs::read_to_string(&candidate)
                    .map_err(|e| Error::Io(e))?;
                let cfg: Config = toml::from_str(&data)
                    .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))?;

                // Validate configuration
                cfg.validate()
                    .map_err(|e| Error::Validation(format!("Configuration validation failed: {:?}", e)))?;

                return Ok(cfg);
            }
        }

        // Return default config if no file found
        Ok(Config::default())
    }

    /// Get bridge name for backward compatibility
    pub fn bridge_name(&self) -> &str {
        &self.bridge.name
    }

    /// Get interfaces path for backward compatibility
    pub fn interfaces_path(&self) -> &str {
        &self.network_manager.interfaces_path
    }

    /// Get include prefixes for backward compatibility
    #[allow(dead_code)]
    pub fn include_prefixes(&self) -> &[String] {
        &self.network_manager.include_prefixes
    }

    /// Get managed block tag for backward compatibility
    pub fn managed_block_tag(&self) -> &str {
        &self.network_manager.managed_block_tag
    }

    /// Get naming template for backward compatibility
    pub fn naming_template(&self) -> &str {
        &self.network_manager.naming_template
    }

    /// Get ledger path for backward compatibility
    pub fn ledger_path(&self) -> &str {
        &self.ledger.path
    }

    /// Get enable rename for backward compatibility
    pub fn enable_rename(&self) -> bool {
        self.network_manager.enable_rename
    }

    /// Get uplink for backward compatibility
    pub fn uplink(&self) -> Option<&str> {
        self.bridge.uplink.as_deref()
    }

    /// Get nm unmanaged for backward compatibility
    pub fn nm_unmanaged(&self) -> &[String] {
        &self.network_manager.unmanaged_devices
    }
}
