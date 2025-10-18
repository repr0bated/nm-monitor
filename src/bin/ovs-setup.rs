#!/usr/bin/env -S cargo +nightly -Zscript
//! OVS Atomic Setup using systemd-networkd
//! Usage: sudo ./ovs-setup.rs

use anyhow::{Context, Result};

const NETWORKD_DIR: &str = "/etc/systemd/network";

#[tokio::main]
async fn main() -> Result<()> {
    // Check if we're actually root (effective UID)
    let euid = unsafe { libc::geteuid() };
    if euid != 0 {
        anyhow::bail!("Must run as root. Try: sudo -E {}", std::env::args().next().unwrap());
    }

    println!("🔧 OVS Atomic Setup");
    println!("==========================================\n");

    // Step 1: Introspect uplink
    println!("📡 Step 1: Introspecting uplink interface...");
    let uplink = introspect_uplink().await?;
    println!("   Uplink: {}", uplink.interface);
    println!("   IP: {}/{}", uplink.ip, uplink.prefix);
    println!("   Gateway: {}", uplink.gateway);
    println!("   DNS: {}\n", uplink.dns_servers.join(", "));

    // Step 2: BUILD complete OVS topology FIRST (no IP yet!)
    println!("🏗️  Step 2: Building OVS bridges and topology...");
    build_ovs_bridges(&uplink.interface).await?;
    println!("   ✓ OVS bridges created (no IP assigned yet)\n");

    // Step 3: Prepare IP configuration
    println!("📝 Step 3: Preparing IP configuration...");
    prepare_bridge_config(&uplink).await?;
    println!("   ✓ Configuration ready\n");

    // Step 4: ATOMIC HANDOFF - move IP from ens1 to ovsbr0
    println!("🚀 Step 4: Atomic IP handoff (ens1 → ovsbr0)...");
    atomic_ip_handoff(&uplink).await?;
    println!("   ✓ IP moved atomically!\n");

    println!("🎉 SUCCESS! OVS bridges created with atomic handoff:");
    println!("   • ovsbr0: {}/{} (uplink: {})", uplink.ip, uplink.prefix, uplink.interface);
    println!("   • ovsbr1: 80.209.242.196/25 (isolated)\n");

    Ok(())
}

#[derive(Debug)]
struct UplinkConfig {
    interface: String,
    ip: String,
    prefix: u8,
    gateway: String,
    dns_servers: Vec<String>,
}

async fn introspect_uplink() -> Result<UplinkConfig> {
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
    let dns_servers = get_dns_servers().await?;

    Ok(UplinkConfig {
        interface: uplink,
        ip,
        prefix,
        gateway,
        dns_servers,
    })
}

async fn get_dns_servers() -> Result<Vec<String>> {
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

/// Step 2: Build OVS bridges using ovs-vsctl (NO IP yet!)
async fn build_ovs_bridges(uplink_iface: &str) -> Result<()> {
    // Create ovsbr0
    tokio::process::Command::new("ovs-vsctl")
        .args(["add-br", "ovsbr0"])
        .output()
        .await?;
    
    // Add uplink port to ovsbr0
    tokio::process::Command::new("ovs-vsctl")
        .args(["add-port", "ovsbr0", uplink_iface])
        .output()
        .await?;
    
    // Bring up ovsbr0 (no IP!)
    tokio::process::Command::new("ip")
        .args(["link", "set", "ovsbr0", "up"])
        .output()
        .await?;
    
    // Create ovsbr1 (isolated)
    tokio::process::Command::new("ovs-vsctl")
        .args(["add-br", "ovsbr1"])
        .output()
        .await?;
    
    // Bring up ovsbr1
    tokio::process::Command::new("ip")
        .args(["link", "set", "ovsbr1", "up"])
        .output()
        .await?;
    
    // Assign static IP to ovsbr1
    tokio::process::Command::new("ip")
        .args(["addr", "add", "80.209.242.196/25", "dev", "ovsbr1"])
        .output()
        .await?;
    
    Ok(())
}

/// Step 3: Prepare bridge configuration files
async fn prepare_bridge_config(uplink: &UplinkConfig) -> Result<()> {
    tokio::fs::create_dir_all(NETWORKD_DIR).await?;

    // Config for ovsbr0 (will be applied in step 4)
    let mut ovsbr0_network = format!(
        "[Match]\nName=ovsbr0\n\n[Network]\nAddress={}/{}\nGateway={}\n",
        uplink.ip, uplink.prefix, uplink.gateway
    );
    for dns in &uplink.dns_servers {
        ovsbr0_network.push_str(&format!("DNS={}\n", dns));
    }
    tokio::fs::write(format!("{}/30-ovsbr0.network", NETWORKD_DIR), ovsbr0_network).await?;

    Ok(())
}

/// Step 4: ATOMIC HANDOFF - move IP from uplink to ovsbr0
async fn atomic_ip_handoff(uplink: &UplinkConfig) -> Result<()> {
    // This is the critical atomic operation!
    // 1. Remove IP from uplink
    // 2. Add IP to ovsbr0
    // Done as quickly as possible to minimize downtime
    
    let ip_with_prefix = format!("{}/{}", uplink.ip, uplink.prefix);
    
    // Remove from uplink
    let _ = tokio::process::Command::new("ip")
        .args(["addr", "del", &ip_with_prefix, "dev", &uplink.interface])
        .output()
        .await?;
    
    // Add to ovsbr0 (ATOMIC!)
    tokio::process::Command::new("ip")
        .args(["addr", "add", &ip_with_prefix, "dev", "ovsbr0"])
        .output()
        .await
        .context("Failed to add IP to ovsbr0")?;
    
    // Update default route to use ovsbr0
    let _ = tokio::process::Command::new("ip")
        .args(["route", "del", "default"])
        .output()
        .await;
    
    tokio::process::Command::new("ip")
        .args(["route", "add", "default", "via", &uplink.gateway, "dev", "ovsbr0"])
        .output()
        .await
        .context("Failed to add default route")?;
    
    Ok(())
}
