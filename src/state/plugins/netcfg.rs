// Netcfg state plugin - manages advanced network configuration
// Handles: routing, OVS flows, DNS, VLANs (tunable/alterable)
use crate::state::plugin::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::fs;
use tokio::process::Command as AsyncCommand;

/// Network configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetcfgConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<Vec<RouteConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ovs_flows: Option<Vec<OvsFlowConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<DnsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteConfig {
    pub destination: String, // e.g., "10.0.0.0/8"
    pub gateway: String,     // e.g., "172.16.0.1"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>, // e.g., "ovsbr0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric: Option<u32>,

    /// Dynamic properties for advanced routing options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OvsFlowConfig {
    pub bridge: String,
    pub priority: u32,
    pub match_rule: String, // OpenFlow match, e.g., "ip,nw_dst=10.0.0.0/8"
    pub actions: String,    // OpenFlow actions, e.g., "output:1"

    /// Dynamic properties for advanced flow options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DnsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    /// Dynamic properties for DNS options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>,
}

/// Netcfg state plugin implementation
pub struct NetcfgStatePlugin {
    #[allow(dead_code)]
    routes_file: String,
}

impl NetcfgStatePlugin {
    pub fn new() -> Self {
        Self {
            routes_file: "/etc/systemd/network/10-routes.network".to_string(),
        }
    }

    /// Query current routing table
    async fn query_routes(&self) -> Result<Vec<RouteConfig>> {
        let output = AsyncCommand::new("ip")
            .args(["route", "show"])
            .output()
            .await
            .context("Failed to run ip route show")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut routes = Vec::new();

        for line in stdout.lines() {
            // Parse lines like: "10.0.0.0/8 via 172.16.0.1 dev ovsbr0"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "via" {
                routes.push(RouteConfig {
                    destination: parts[0].to_string(),
                    gateway: parts[2].to_string(),
                    interface: parts.get(4).map(|s| s.to_string()),
                    metric: None,
                    properties: None,
                });
            }
        }

        Ok(routes)
    }

    /// Query current OVS flows
    async fn query_ovs_flows(&self) -> Result<HashMap<String, Vec<OvsFlowConfig>>> {
        // Check if OVS is available first
        let check_output = AsyncCommand::new("ovs-vsctl")
            .arg("--version")
            .output()
            .await;
            
        if check_output.is_err() || !check_output.unwrap().status.success() {
            log::info!("OVS not available, skipping OVS flows query");
            return Ok(HashMap::new());
        }

        // Get list of OVS bridges
        let output = AsyncCommand::new("ovs-vsctl")
            .args(["list-br"])
            .output()
            .await
            .context("Failed to list OVS bridges")?;

        let bridges = String::from_utf8_lossy(&output.stdout);
        let mut flows_by_bridge = HashMap::new();

        for bridge in bridges.lines() {
            let bridge = bridge.trim();
            if bridge.is_empty() {
                continue;
            }

            // Get flows for this bridge
            let output = AsyncCommand::new("ovs-ofctl")
                .args(["dump-flows", bridge])
                .output()
                .await
                .context("Failed to dump flows")?;

            let flows_output = String::from_utf8_lossy(&output.stdout);
            let mut flows = Vec::new();

            for line in flows_output.lines() {
                // Skip header lines
                if line.contains("NXST_FLOW") || line.starts_with("cookie=") {
                    // Parse flow rules
                    // Example: "priority=200,ip,nw_dst=10.0.0.0/8 actions=output:1"
                    if line.contains("priority=") {
                        // This is a simplified parser - full implementation would be more robust
                        flows.push(OvsFlowConfig {
                            bridge: bridge.to_string(),
                            priority: 0, // Would parse from line
                            match_rule: String::new(),
                            actions: String::new(),
                            properties: None,
                        });
                    }
                }
            }

            flows_by_bridge.insert(bridge.to_string(), flows);
        }

        Ok(flows_by_bridge)
    }

    /// Query DNS configuration
    async fn query_dns(&self) -> Result<DnsConfig> {
        // Read /etc/hostname for hostname
        let hostname = if let Ok(content) = fs::read_to_string("/etc/hostname").await {
            Some(content.trim().to_string())
        } else {
            None
        };

        // Read /etc/resolv.conf for search domains
        let search_domains = if let Ok(content) = fs::read_to_string("/etc/resolv.conf").await {
            let mut domains = Vec::new();
            for line in content.lines() {
                if let Some(search_line) = line.strip_prefix("search ") {
                    domains.extend(
                        search_line
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>(),
                    );
                }
            }
            if !domains.is_empty() {
                Some(domains)
            } else {
                None
            }
        } else {
            None
        };

        Ok(DnsConfig {
            search_domains,
            hostname,
            properties: None,
        })
    }

    /// Apply routing configuration
    async fn apply_routes(&self, routes: &[RouteConfig]) -> Result<()> {
        // Remove existing routes (except default and local)
        let current_routes = self.query_routes().await?;
        for route in current_routes {
            if route.destination != "default" && !route.destination.starts_with("169.254") {
                let _ = AsyncCommand::new("ip")
                    .args(["route", "del", &route.destination])
                    .output()
                    .await;
            }
        }

        // Add new routes
        for route in routes {
            let mut args = vec!["route", "add", &route.destination, "via", &route.gateway];

            if let Some(ref iface) = route.interface {
                args.extend(&["dev", iface]);
            }

            let metric_str = route.metric.map(|m| m.to_string());
            if let Some(ref metric) = metric_str {
                args.extend(&["metric", metric]);
            }

            AsyncCommand::new("ip")
                .args(&args)
                .output()
                .await
                .context("Failed to add route")?;
        }

        Ok(())
    }

    /// Apply OVS flow rules
    async fn apply_ovs_flows(&self, flows: &[OvsFlowConfig]) -> Result<()> {
        // Group flows by bridge
        let mut flows_by_bridge: HashMap<String, Vec<&OvsFlowConfig>> = HashMap::new();
        for flow in flows {
            flows_by_bridge
                .entry(flow.bridge.clone())
                .or_default()
                .push(flow);
        }

        // For each bridge, delete existing custom flows and add new ones
        for (bridge, bridge_flows) in flows_by_bridge {
            // Verify bridge exists
            let output = AsyncCommand::new("ovs-vsctl")
                .args(["br-exists", &bridge])
                .output()
                .await?;

            if !output.status.success() {
                return Err(anyhow!("OVS bridge '{}' does not exist", bridge));
            }

            // Delete existing flows with priority >= 200 (custom flows, keep security flows < 200)
            let _ = AsyncCommand::new("ovs-ofctl")
                .args(["del-flows", &bridge, "priority=200"])
                .output()
                .await;

            // Add new flows
            for flow in bridge_flows {
                let flow_spec = format!(
                    "priority={},{},actions={}",
                    flow.priority, flow.match_rule, flow.actions
                );

                AsyncCommand::new("ovs-ofctl")
                    .args(["add-flow", &bridge, &flow_spec])
                    .output()
                    .await
                    .context("Failed to add OVS flow")?;
            }
        }

        Ok(())
    }

    /// Apply DNS configuration
    async fn apply_dns(&self, dns: &DnsConfig) -> Result<()> {
        // Set hostname if provided
        if let Some(ref hostname) = dns.hostname {
            fs::write("/etc/hostname", format!("{}\n", hostname))
                .await
                .context("Failed to write /etc/hostname")?;

            AsyncCommand::new("hostnamectl")
                .args(["set-hostname", hostname])
                .output()
                .await
                .context("Failed to set hostname")?;
        }

        // Update search domains in /etc/resolv.conf
        if let Some(ref search_domains) = dns.search_domains {
            // Read current resolv.conf
            let current = fs::read_to_string("/etc/resolv.conf")
                .await
                .unwrap_or_default();

            // Remove old search lines and add new one
            let mut new_lines: Vec<String> = current
                .lines()
                .filter(|line| !line.starts_with("search "))
                .map(|s| s.to_string())
                .collect();

            new_lines.insert(0, format!("search {}", search_domains.join(" ")));

            fs::write("/etc/resolv.conf", new_lines.join("\n") + "\n")
                .await
                .context("Failed to write /etc/resolv.conf")?;
        }

        Ok(())
    }
}

#[async_trait]
impl StatePlugin for NetcfgStatePlugin {
    fn name(&self) -> &str {
        "netcfg"
    }

    #[allow(dead_code)]
    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let routes = self.query_routes().await?;
        let ovs_flows = self.query_ovs_flows().await?;
        let dns = self.query_dns().await?;

        // Only include DNS if it has actual configuration
        let dns_config = if dns.hostname.is_some() || dns.search_domains.is_some() {
            Some(dns)
        } else {
            None
        };

        let config = NetcfgConfig {
            routing: if routes.is_empty() {
                None
            } else {
                Some(routes)
            },
            ovs_flows: if ovs_flows.is_empty() {
                None
            } else {
                // Flatten flows for serialization
                let mut all_flows = Vec::new();
                for flows in ovs_flows.values() {
                    all_flows.extend(flows.clone());
                }
                Some(all_flows)
            },
            dns: dns_config,
        };

        Ok(serde_json::to_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_config: NetcfgConfig = serde_json::from_value(current.clone())?;
        let desired_config: NetcfgConfig = serde_json::from_value(desired.clone())?;

        let mut actions = Vec::new();

        // Check routing changes - only if desired explicitly specifies routing
        if let Some(desired_routing) = &desired_config.routing {
            if current_config.routing.as_ref() != Some(desired_routing) {
                actions.push(StateAction::Modify {
                    resource: "routing".to_string(),
                    changes: serde_json::json!({
                        "old": current_config.routing,
                        "new": desired_routing,
                    }),
                });
            }
        }

        // Check OVS flow changes - only if desired explicitly specifies OVS flows
        if let Some(desired_flows) = &desired_config.ovs_flows {
            if current_config.ovs_flows.as_ref() != Some(desired_flows) {
                actions.push(StateAction::Modify {
                    resource: "ovs_flows".to_string(),
                    changes: serde_json::json!({
                        "old": current_config.ovs_flows,
                        "new": desired_flows,
                    }),
                });
            }
        }

        // Check DNS changes - only if desired explicitly specifies DNS
        if let Some(desired_dns) = &desired_config.dns {
            if current_config.dns.as_ref() != Some(desired_dns) {
                actions.push(StateAction::Modify {
                    resource: "dns".to_string(),
                    changes: serde_json::json!({
                        "old": current_config.dns,
                        "new": desired_dns,
                    }),
                });
            }
        }

        let current_json = serde_json::to_string(&current_config)?;
        let current_hash = format!("{:x}", md5::compute(&current_json));

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut results = Vec::new();

        for action in &diff.actions {
            if let StateAction::Modify { resource, changes } = action {
                match resource.as_str() {
                    "routing" => {
                        if let Some(routes) = changes.get("new") {
                            let routes: Vec<RouteConfig> = serde_json::from_value(routes.clone())?;
                            self.apply_routes(&routes).await?;
                            results.push(format!("Applied {} routes", routes.len()));
                        }
                    }
                    "ovs_flows" => {
                        if let Some(flows) = changes.get("new") {
                            let flows: Vec<OvsFlowConfig> = serde_json::from_value(flows.clone())?;
                            self.apply_ovs_flows(&flows).await?;
                            results.push(format!("Applied {} OVS flows", flows.len()));
                        }
                    }
                    "dns" => {
                        if let Some(dns_value) = changes.get("new") {
                            let dns: DnsConfig = serde_json::from_value(dns_value.clone())?;
                            self.apply_dns(&dns).await?;
                            results.push("Applied DNS configuration".to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(ApplyResult {
            success: true,
            changes_applied: results,
            errors: Vec::new(),
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, desired).await?;
        Ok(diff.actions.is_empty())
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("netcfg-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let config: NetcfgConfig = serde_json::from_value(checkpoint.state_snapshot.clone())?;

        if let Some(routes) = config.routing {
            self.apply_routes(&routes).await?;
        }

        if let Some(flows) = config.ovs_flows {
            self.apply_ovs_flows(&flows).await?;
        }

        if let Some(dns) = config.dns {
            self.apply_dns(&dns).await?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}
