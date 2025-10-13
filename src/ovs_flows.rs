use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// OVS Flow Rule Management System
/// Implements the advanced traffic engineering described in the whitepaper

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowPriority {
    ContainerSpecific = 400,
    ApplicationAware = 350,
    PrivacyRouting = 300,
    GeographicRouting = 250,
    SecurityPolicy = 200,
    VlanIsolation = 180,
    DdosProtection = 150,
    Default = 0,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowAction {
    Output(String),               // output:port
    SetField(String),             // set_field:value->reg
    SetQueue(u32),                // set_queue:queue_id
    OutputWithQueue(String, u32), // set_queue:queue_id,output:port
    Drop,                         // drop
    Normal,                       // NORMAL
    Local,                        // LOCAL
    Fragment,                     // fragment
    RateLimit(u32),               // set_field:value->rate
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowMatch {
    IpSrc(String),     // nw_src=ip
    IpDst(String),     // nw_dst=ip
    TcpDst(u16),       // tp_dst=port
    UdpDst(u16),       // tp_dst=port
    Vlan(u16),         // dl_vlan=vlan_id
    InPort(String),    // in_port=port
    ArpTarget(String), // arp_tpa=ip
    Established,       // ct_state=+est
    NewConnection,     // ct_state=-est
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRule {
    pub priority: FlowPriority,
    pub matches: Vec<FlowMatch>,
    pub actions: Vec<FlowAction>,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Application {
    Tor,
    Signal,
    Browser,
    Streaming,
    Anonymous,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum PrivacyPath {
    WireGuard,
    Warp,
    XrayReality,
}

pub struct OvsFlowManager {
    bridge_name: String,
}

impl OvsFlowManager {
    pub fn new(bridge_name: String) -> Self {
        Self { bridge_name }
    }

    /// Clear all existing flows on the bridge
    pub fn clear_all_flows(&self) -> Result<()> {
        info!("Clearing all flows on bridge {}", self.bridge_name);
        let output = Command::new("ovs-ofctl")
            .args(["del-flows", &self.bridge_name])
            .output()
            .context("Failed to clear OVS flows")?;

        if !output.status.success() {
            warn!(
                "Failed to clear flows: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Add a single flow rule
    pub fn add_flow(&self, rule: &FlowRule) -> Result<()> {
        let flow_string = self.build_flow_string(rule);
        info!(
            "Adding flow: {} ({}: {})",
            flow_string,
            rule.priority.clone() as u32,
            rule.comment
        );

        let output = Command::new("ovs-ofctl")
            .args(["add-flow", &self.bridge_name, &flow_string])
            .output()
            .context("Failed to add OVS flow")?;

        if !output.status.success() {
            warn!(
                "Failed to add flow '{}': {}",
                flow_string,
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(anyhow::anyhow!("Failed to add flow"));
        }

        Ok(())
    }

    /// Add multiple flow rules
    pub fn add_flows(&self, rules: &[FlowRule]) -> Result<()> {
        for rule in rules {
            self.add_flow(rule)?;
        }
        Ok(())
    }

    /// Remove a specific flow rule
    #[allow(dead_code)]
    pub fn remove_flow(&self, rule: &FlowRule) -> Result<()> {
        let flow_string = self.build_flow_string(rule);
        info!("Removing flow: {}", flow_string);

        let output = Command::new("ovs-ofctl")
            .args(["del-flows", &self.bridge_name, &flow_string])
            .output()
            .context("Failed to remove OVS flow")?;

        if !output.status.success() {
            warn!(
                "Failed to remove flow '{}': {}",
                flow_string,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Dump all current flows
    pub fn dump_flows(&self) -> Result<String> {
        let output = Command::new("ovs-ofctl")
            .args(["dump-flows", &self.bridge_name])
            .output()
            .context("Failed to dump OVS flows")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Setup container-specific routing rules
    pub fn setup_container_routing(
        &self,
        container_ip: &str,
        container_port: &str,
        register_id: u32,
    ) -> Result<()> {
        info!(
            "Setting up container-specific routing for {} (port: {}, reg: {})",
            container_ip, container_port, register_id
        );

        let rules = vec![FlowRule {
            priority: FlowPriority::ContainerSpecific,
            matches: vec![FlowMatch::IpSrc(container_ip.to_string())],
            actions: vec![
                FlowAction::SetField(format!("{}->reg0", register_id)),
                FlowAction::Output(container_port.to_string()),
            ],
            comment: format!(
                "Container {} routing via register {}",
                container_ip, register_id
            ),
        }];

        self.add_flows(&rules)
    }

    /// Setup application-aware routing rules
    pub fn setup_application_routing(&self, output_port: &str) -> Result<()> {
        info!("Setting up application-aware routing rules");

        let rules = vec![
            // HTTP traffic gets priority queue
            FlowRule {
                priority: FlowPriority::ApplicationAware,
                matches: vec![FlowMatch::TcpDst(80)],
                actions: vec![FlowAction::OutputWithQueue(output_port.to_string(), 1)],
                comment: "HTTP traffic priority routing".to_string(),
            },
            // HTTPS traffic gets priority queue
            FlowRule {
                priority: FlowPriority::ApplicationAware,
                matches: vec![FlowMatch::TcpDst(443)],
                actions: vec![FlowAction::OutputWithQueue(output_port.to_string(), 1)],
                comment: "HTTPS traffic priority routing".to_string(),
            },
            // SSH traffic gets normal queue
            FlowRule {
                priority: FlowPriority::ApplicationAware,
                matches: vec![FlowMatch::TcpDst(22)],
                actions: vec![FlowAction::OutputWithQueue(output_port.to_string(), 0)],
                comment: "SSH traffic normal routing".to_string(),
            },
        ];

        self.add_flows(&rules)
    }

    /// Setup privacy routing rules
    #[allow(dead_code)]
    pub fn setup_privacy_routing(&self, dns_port: &str, https_port: &str) -> Result<()> {
        info!("Setting up privacy routing rules");

        let rules = vec![
            // DNS over VPN tunnel
            FlowRule {
                priority: FlowPriority::PrivacyRouting,
                matches: vec![FlowMatch::UdpDst(53)],
                actions: vec![FlowAction::Output(dns_port.to_string())],
                comment: "DNS privacy routing".to_string(),
            },
            // HTTPS over privacy tunnel
            FlowRule {
                priority: FlowPriority::PrivacyRouting,
                matches: vec![FlowMatch::TcpDst(443)],
                actions: vec![FlowAction::Output(https_port.to_string())],
                comment: "HTTPS privacy routing".to_string(),
            },
        ];

        self.add_flows(&rules)
    }

    /// Setup geographic routing rules
    #[allow(dead_code)]
    pub fn setup_geographic_routing(&self, cidr: &str, output_port: &str) -> Result<()> {
        info!("Setting up geographic routing for {}", cidr);

        let rules = vec![FlowRule {
            priority: FlowPriority::GeographicRouting,
            matches: vec![FlowMatch::IpDst(cidr.to_string())],
            actions: vec![FlowAction::Output(output_port.to_string())],
            comment: format!("Geographic routing for {}", cidr),
        }];

        self.add_flows(&rules)
    }

    /// Setup security policy rules
    #[allow(dead_code)]
    pub fn setup_security_policies(&self) -> Result<()> {
        info!("Setting up security policy rules");

        let rules = vec![
            // Allow established connections
            FlowRule {
                priority: FlowPriority::SecurityPolicy,
                matches: vec![FlowMatch::Established],
                actions: vec![FlowAction::Output("allow".to_string())], // This would need proper action
                comment: "Allow established connections".to_string(),
            },
            // Drop new connections by default
            FlowRule {
                priority: FlowPriority::SecurityPolicy,
                matches: vec![FlowMatch::NewConnection],
                actions: vec![FlowAction::Drop],
                comment: "Drop new connections by default".to_string(),
            },
        ];

        // Note: The "allow" action above is conceptual - OVS doesn't have an "allow" action
        // This would need to be implemented differently
        self.add_flows(&rules)
    }

    /// Setup DDoS protection rules
    #[allow(dead_code)]
    pub fn setup_ddos_protection(&self, suspicious_cidr: &str, rate_limit: u32) -> Result<()> {
        info!("Setting up DDoS protection for {}", suspicious_cidr);

        let rules = vec![FlowRule {
            priority: FlowPriority::DdosProtection,
            matches: vec![FlowMatch::IpSrc(suspicious_cidr.to_string())],
            actions: vec![FlowAction::RateLimit(rate_limit)],
            comment: format!("DDoS protection for {}", suspicious_cidr),
        }];

        self.add_flows(&rules)
    }

    /// Setup VLAN-based isolation
    #[allow(dead_code)]
    pub fn setup_vlan_isolation(
        &self,
        vlan_id: u16,
        queue_id: u32,
        output_port: &str,
    ) -> Result<()> {
        info!("Setting up VLAN isolation for VLAN {}", vlan_id);

        let rules = vec![FlowRule {
            priority: FlowPriority::VlanIsolation,
            matches: vec![FlowMatch::Vlan(vlan_id)],
            actions: vec![FlowAction::OutputWithQueue(
                output_port.to_string(),
                queue_id,
            )],
            comment: format!("VLAN {} isolation with QoS", vlan_id),
        }];

        self.add_flows(&rules)
    }

    /// Generate intelligent routing based on application detection
    #[allow(dead_code)]
    pub fn route_privacy_traffic(&self, packet_app: Application) -> PrivacyPath {
        match packet_app {
            Application::Tor | Application::Signal => PrivacyPath::XrayReality,
            Application::Browser => PrivacyPath::Warp,
            Application::Streaming => PrivacyPath::WireGuard,
            Application::Anonymous => PrivacyPath::XrayReality,
            Application::General => PrivacyPath::WireGuard,
        }
    }

    /// Setup basic routing infrastructure
    pub fn setup_basic_routing(
        &self,
        container_network: &str,
        physical_interface: &str,
        internal_port: &str,
    ) -> Result<()> {
        info!("Setting up basic routing infrastructure");

        let rules = vec![
            // Route traffic from container network to physical interface
            FlowRule {
                priority: FlowPriority::Default,
                matches: vec![
                    FlowMatch::InPort(internal_port.to_string()),
                    FlowMatch::IpSrc(container_network.to_string()),
                ],
                actions: vec![FlowAction::Output(physical_interface.to_string())],
                comment: format!(
                    "Container {} -> physical {}",
                    container_network, physical_interface
                ),
            },
            // Route traffic from physical interface to container network
            FlowRule {
                priority: FlowPriority::Default,
                matches: vec![
                    FlowMatch::InPort(physical_interface.to_string()),
                    FlowMatch::IpDst(container_network.to_string()),
                ],
                actions: vec![FlowAction::Output(internal_port.to_string())],
                comment: format!(
                    "Physical {} -> container {}",
                    physical_interface, container_network
                ),
            },
            // Allow local traffic within container network
            FlowRule {
                priority: FlowPriority::ApplicationAware,
                matches: vec![
                    FlowMatch::InPort(internal_port.to_string()),
                    FlowMatch::IpSrc(container_network.to_string()),
                    FlowMatch::IpDst(container_network.to_string()),
                ],
                actions: vec![FlowAction::Output(internal_port.to_string())],
                comment: "Local container network traffic".to_string(),
            },
            // Default action for other traffic
            FlowRule {
                priority: FlowPriority::Default,
                matches: vec![],
                actions: vec![FlowAction::Normal],
                comment: "Default NORMAL action".to_string(),
            },
        ];

        self.add_flows(&rules)
    }

    /// Build flow string from FlowRule
    fn build_flow_string(&self, rule: &FlowRule) -> String {
        let mut parts = vec![format!("priority={}", rule.priority.clone() as u32)];

        // Add matches
        for match_condition in &rule.matches {
            match match_condition {
                FlowMatch::IpSrc(ip) => parts.push(format!("ip,nw_src={}", ip)),
                FlowMatch::IpDst(ip) => parts.push(format!("ip,nw_dst={}", ip)),
                FlowMatch::TcpDst(port) => parts.push(format!("tcp,tp_dst={}", port)),
                FlowMatch::UdpDst(port) => parts.push(format!("udp,tp_dst={}", port)),
                FlowMatch::Vlan(vlan) => parts.push(format!("dl_vlan={}", vlan)),
                FlowMatch::InPort(port) => parts.push(format!("in_port={}", port)),
                FlowMatch::ArpTarget(ip) => parts.push(format!("arp,arp_tpa={}", ip)),
                FlowMatch::Established => parts.push("ct_state=+est".to_string()),
                FlowMatch::NewConnection => parts.push("ct_state=-est".to_string()),
            }
        }

        // Add actions
        let mut actions = Vec::new();
        for action in &rule.actions {
            match action {
                FlowAction::Output(port) => actions.push(format!("output:{}", port)),
                FlowAction::SetField(field) => actions.push(format!("set_field:{}", field)),
                FlowAction::SetQueue(queue) => actions.push(format!("set_queue:{}", queue)),
                FlowAction::OutputWithQueue(port, queue) => {
                    actions.push(format!("set_queue:{}", queue));
                    actions.push(format!("output:{}", port));
                }
                FlowAction::Drop => actions.push("drop".to_string()),
                FlowAction::Normal => actions.push("NORMAL".to_string()),
                FlowAction::Local => actions.push("LOCAL".to_string()),
                FlowAction::Fragment => actions.push("fragment".to_string()),
                FlowAction::RateLimit(rate) => actions.push(format!("set_field:{}->rate", rate)),
            }
        }

        parts.push(format!("actions={}", actions.join(",")));
        parts.join(",")
    }

    /// Generate container flow rules automatically
    #[allow(dead_code)]
    pub fn generate_container_flows(
        &self,
        container_ip: &str,
        container_port: u16,
    ) -> Vec<FlowRule> {
        vec![
            FlowRule {
                priority: FlowPriority::ContainerSpecific,
                matches: vec![FlowMatch::IpSrc(container_ip.to_string())],
                actions: vec![
                    FlowAction::SetField("1->reg0".to_string()),
                    FlowAction::Output(container_port.to_string()),
                ],
                comment: format!("Container {} routing", container_ip),
            },
            FlowRule {
                priority: FlowPriority::ApplicationAware,
                matches: vec![
                    FlowMatch::IpSrc(container_ip.to_string()),
                    FlowMatch::TcpDst(80),
                ],
                actions: vec![
                    FlowAction::SetQueue(1),
                    FlowAction::Output(container_port.to_string()),
                ],
                comment: format!("Container {} HTTP priority", container_ip),
            },
            FlowRule {
                priority: FlowPriority::ApplicationAware,
                matches: vec![
                    FlowMatch::IpSrc(container_ip.to_string()),
                    FlowMatch::TcpDst(22),
                ],
                actions: vec![
                    FlowAction::SetQueue(0),
                    FlowAction::Output(container_port.to_string()),
                ],
                comment: format!("Container {} SSH normal", container_ip),
            },
        ]
    }
}
