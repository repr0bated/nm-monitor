// OVS Atomic Setup using systemd-networkd native support
// Uses zbus to communicate with org.freedesktop.network1

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use zbus::Connection;

const NETWORKD_DIR: &str = "/etc/systemd/network";

/// Atomic OVS bridge setup using systemd-networkd
pub struct OvsAtomicSetup {
    conn: Connection,
}

impl OvsAtomicSetup {
    /// Create new OVS atomic setup handler
    pub async fn new() -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;
        
        Ok(Self { conn })
    }

    /// Introspect uplink interface and get network configuration
    pub async fn introspect_uplink(&self) -> Result<UplinkConfig> {
        // Get interface with default route
        let output = tokio::process::Command::new("ip")
            .args(["-o", "-4", "route", "show", "default"])
            .output()
            .await?;
        
        let route_output = String::from_utf8_lossy(&output.stdout);
        let uplink = route_output
            .split_whitespace()
            .nth(4)
            .context("Could not find uplink interface")?
            .to_string();

        // Get IP address
        let output = tokio::process::Command::new("ip")
            .args(["-o", "-4", "addr", "show", &uplink])
            .output()
            .await?;
        
        let addr_output = String::from_utf8_lossy(&output.stdout);
        let addr_with_prefix = addr_output
            .split_whitespace()
            .find(|s| s.contains('/'))
            .context("Could not find IP address")?;
        
        let parts: Vec<&str> = addr_with_prefix.split('/').collect();
        let ip = parts[0].to_string();
        let prefix: u8 = parts[1].parse()?;

        // Get gateway
        let gateway = route_output
            .split_whitespace()
            .nth(2)
            .context("Could not find gateway")?
            .to_string();

        // Get DNS servers
        let dns_servers = self.get_dns_servers().await?;

        Ok(UplinkConfig {
            interface: uplink,
            ip,
            prefix,
            gateway,
            dns_servers,
        })
    }

    /// Get DNS servers from /etc/resolv.conf
    async fn get_dns_servers(&self) -> Result<Vec<String>> {
        let resolv = tokio::fs::read_to_string("/etc/resolv.conf").await?;
        let dns_servers: Vec<String> = resolv
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("nameserver") {
                    trimmed.split_whitespace().nth(1).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        if dns_servers.is_empty() {
            Ok(vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()])
        } else {
            Ok(dns_servers)
        }
    }

    /// Create systemd-networkd configuration files for OVS bridges
    pub async fn create_ovs_configs(&self, uplink: &UplinkConfig) -> Result<()> {
        // Ensure directory exists
        tokio::fs::create_dir_all(NETWORKD_DIR).await?;

        log::info!("Creating OVS bridge configurations");

        // 1. Create ovsbr0 bridge device
        let ovsbr0_netdev = format!(
            "# OVS Bridge 0 - Primary bridge with uplink\n\
             # Created: {}\n\
             [NetDev]\n\
             Name=ovsbr0\n\
             Kind=openvswitch\n",
            chrono::Utc::now()
        );
        tokio::fs::write(
            format!("{}/10-ovsbr0.netdev", NETWORKD_DIR),
            ovsbr0_netdev,
        )
        .await?;

        // 2. Attach uplink to bridge
        let uplink_network = format!(
            "# Uplink interface - attached to ovsbr0\n\
             # IP configuration moves to bridge\n\
             [Match]\n\
             Name={}\n\
             \n\
             [Network]\n\
             Bridge=ovsbr0\n\
             IgnoreCarrierLoss=yes\n",
            uplink.interface
        );
        tokio::fs::write(
            format!("{}/20-{}.network", NETWORKD_DIR, uplink.interface),
            uplink_network,
        )
        .await?;

        // 3. Configure ovsbr0 with IP (atomic handoff!)
        let mut ovsbr0_network = format!(
            "# OVS Bridge 0 network configuration\n\
             # IP moved from {}\n\
             [Match]\n\
             Name=ovsbr0\n\
             \n\
             [Network]\n\
             Address={}/{}\n\
             Gateway={}\n",
            uplink.interface, uplink.ip, uplink.prefix, uplink.gateway
        );

        for dns in &uplink.dns_servers {
            ovsbr0_network.push_str(&format!("DNS={}\n", dns));
        }

        ovsbr0_network.push_str("IgnoreCarrierLoss=yes\nConfigureWithoutCarrier=yes\n");

        tokio::fs::write(
            format!("{}/30-ovsbr0.network", NETWORKD_DIR),
            ovsbr0_network,
        )
        .await?;

        // 4. Create ovsbr1 (isolated bridge)
        let ovsbr1_netdev = format!(
            "# OVS Bridge 1 - Isolated bridge for containers\n\
             # Created: {}\n\
             [NetDev]\n\
             Name=ovsbr1\n\
             Kind=openvswitch\n",
            chrono::Utc::now()
        );
        tokio::fs::write(
            format!("{}/11-ovsbr1.netdev", NETWORKD_DIR),
            ovsbr1_netdev,
        )
        .await?;

        let ovsbr1_network = "# OVS Bridge 1 network configuration\n\
             [Match]\n\
             Name=ovsbr1\n\
             \n\
             [Network]\n\
             Address=80.209.242.196/25\n\
             Gateway=80.209.242.129\n\
             DNS=8.8.8.8\n\
             DNS=8.8.4.4\n\
             IgnoreCarrierLoss=yes\n\
             ConfigureWithoutCarrier=yes\n";

        tokio::fs::write(
            format!("{}/31-ovsbr1.network", NETWORKD_DIR),
            ovsbr1_network,
        )
        .await?;

        log::info!("OVS configuration files created");
        Ok(())
    }

    /// Trigger atomic handoff by calling systemd-networkd.Reload via D-Bus
    pub async fn trigger_atomic_handoff(&self) -> Result<()> {
        log::info!("Triggering atomic handoff via D-Bus...");

        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.freedesktop.network1",
            "/org/freedesktop/network1",
            "org.freedesktop.network1.Manager",
        )
        .await
        .context("Failed to create systemd-networkd proxy")?;

        // Call Reload method - this is the atomic handoff!
        proxy
            .call_method("Reload", &())
            .await
            .context("Failed to call Reload on systemd-networkd")?;

        log::info!("Atomic handoff triggered successfully");

        // Wait for networkd to apply changes
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        Ok(())
    }

    /// Verify that OVS bridges were created
    pub async fn verify_ovs_bridges(&self) -> Result<()> {
        log::info!("Verifying OVS bridges...");

        // Check ovsbr0
        let output = tokio::process::Command::new("ovs-vsctl")
            .args(["br-exists", "ovsbr0"])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("ovsbr0 bridge was not created");
        }

        // Check ovsbr1
        let output = tokio::process::Command::new("ovs-vsctl")
            .args(["br-exists", "ovsbr1"])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("ovsbr1 bridge was not created");
        }

        log::info!("OVS bridges verified successfully");
        Ok(())
    }

    /// Full atomic setup workflow
    pub async fn setup(&self) -> Result<UplinkConfig> {
        log::info!("Starting OVS atomic setup");

        // 1. Introspect uplink
        let uplink = self.introspect_uplink().await?;
        log::info!("Introspected uplink: {}", uplink.interface);

        // 2. Create configuration files
        self.create_ovs_configs(&uplink).await?;

        // 3. Trigger atomic handoff
        self.trigger_atomic_handoff().await?;

        // 4. Verify
        self.verify_ovs_bridges().await?;

        log::info!("OVS atomic setup complete!");
        Ok(uplink)
    }
}

/// Uplink configuration
#[derive(Debug, Clone)]
pub struct UplinkConfig {
    pub interface: String,
    pub ip: String,
    pub prefix: u8,
    pub gateway: String,
    pub dns_servers: Vec<String>,
}
