//! zbus-based OVS Bridge Manager
//! Creates bridges programmatically and derives declarative state

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::process::Command;

/// Bridge configuration derived from zbus operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeState {
    pub name: String,
    pub uplink: Option<String>,
    pub stp_enabled: bool,
    pub ports: Vec<String>,
    pub ip_config: Option<IpConfig>,
}

/// IP configuration for bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpConfig {
    pub address: String,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
}

/// zbus-based bridge manager
pub struct ZbusBridgeManager {
    bridges: HashMap<String, BridgeState>,
}

impl ZbusBridgeManager {
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
        }
    }

    /// Create OVS bridge and capture state
    pub async fn create_bridge(&mut self, name: &str, uplink: Option<&str>) -> Result<BridgeState> {
        // 1. Create OVS bridge
        self.run_ovs_command(&["add-br", name]).await?;
        
        // 2. Disable STP
        self.run_ovs_command(&["set", "bridge", name, "stp_enable=false"]).await?;
        
        // 3. Add uplink if specified
        let mut ports = Vec::new();
        if let Some(uplink) = uplink {
            self.run_ovs_command(&["add-port", name, uplink]).await?;
            ports.push(uplink.to_string());
        }
        
        // 4. Create state representation
        let state = BridgeState {
            name: name.to_string(),
            uplink: uplink.map(|s| s.to_string()),
            stp_enabled: false,
            ports,
            ip_config: None, // Set later via configure_ip
        };
        
        self.bridges.insert(name.to_string(), state.clone());
        Ok(state)
    }

    /// Configure IP on existing bridge
    pub async fn configure_ip(&mut self, bridge_name: &str, config: IpConfig) -> Result<()> {
        if let Some(bridge) = self.bridges.get_mut(bridge_name) {
            // Configure IP via systemd-networkd or direct commands
            // This would create appropriate network files or use nmcli
            
            bridge.ip_config = Some(config);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Bridge {} not found", bridge_name))
        }
    }

    /// Get current state for all bridges
    pub fn get_state(&self) -> HashMap<String, BridgeState> {
        self.bridges.clone()
    }

    /// Convert to declarative YAML state
    pub fn to_declarative_state(&self) -> Result<String> {
        // Convert bridge state to YAML format for persistence
        serde_yaml::to_string(&self.bridges)
            .context("Failed to serialize bridge state")
    }

    /// Load state from declarative format
    pub fn from_declarative_state(yaml: &str) -> Result<Self> {
        let bridges: HashMap<String, BridgeState> = serde_yaml::from_str(yaml)
            .context("Failed to deserialize bridge state")?;
        
        Ok(Self { bridges })
    }

    async fn run_ovs_command(&self, args: &[&str]) -> Result<()> {
        let output = Command::new("ovs-vsctl")
            .args(args)
            .output()
            .await
            .context("Failed to run ovs-vsctl command")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ovs-vsctl failed: {}", stderr));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_creation() {
        let mut manager = ZbusBridgeManager::new();
        
        // This would create a real bridge in integration tests
        let state = manager.create_bridge("testbr0", Some("eth0")).await;
        
        // Verify state capture
        assert!(state.is_ok());
        let state = state.unwrap();
        assert_eq!(state.name, "testbr0");
        assert_eq!(state.uplink, Some("eth0".to_string()));
        assert!(!state.stp_enabled);
    }
}
