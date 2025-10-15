#![allow(dead_code, unused_imports)]
//! OVS Port Agent - Main Application

mod command;
mod error;
mod networkd_dbus;
mod plugin_footprint;
mod streaming_blockchain;
mod config;
mod fuse;
mod interfaces;
mod ledger;
mod link;
mod logging;
mod naming;
mod netlink;
mod nm_bridge;
mod nm_config;
mod nm_ports;
mod nm_query;
mod rpc;
mod services;
mod state;
mod systemd_dbus;
mod systemd_net;
mod zbus_networkd;

use crate::error::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "ovs-port-agent", version, about = "OVS container port agent", long_about=None)]
struct Cli {
    /// Path to config file is (default: /etc/ovs-port-agent/config.toml)
    #[arg(global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the long-lived agent service
    Run,
    /// Show naming example for a container and index
    Name { container: String, index: u16 },
    /// Create a container interface with proper vi{VMID} naming
    CreateInterface {
        /// Raw interface name (e.g., veth-123-eth0)
        raw_ifname: String,
        /// Container identifier
        container_id: String,
        /// VM ID number
        vmid: u32,
    },
    /// Remove a container interface
    RemoveInterface {
        /// Interface name to remove (e.g., vi100)
        interface_name: String,
    },
    /// List OVS ports on the configured bridge
    List,
    /// Comprehensive systemd-networkd introspection and debugging
    IntrospectSystemd,
    /// Apply declarative state from YAML file
    ApplyState {
        /// Path to state YAML file
        state_file: std::path::PathBuf,
    },
    /// Query current system state
    QueryState {
        /// Optional plugin name (network, filesystem, etc.)
        plugin: Option<String>,
    },
    /// Show diff between current and desired state
    ShowDiff {
        /// Path to desired state YAML file
        state_file: std::path::PathBuf,
    },
}

/// Convert anyhow::Result to our custom Result type
fn convert_result<T>(result: anyhow::Result<T>) -> Result<T> {
    result.map_err(|e| crate::error::Error::Internal(e.to_string()))
}

/// Initialize logging with tracing
fn init_logging() -> Result<()> {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::from_default_env()
        .add_directive("ovs_port_agent=info".parse().unwrap())
        .add_directive("tower_http=debug".parse().unwrap());

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        crate::error::Error::Internal(format!("Failed to set tracing subscriber: {}", e))
    })?;

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Initialize structured logging
    init_logging()?;

    let args = Cli::parse();
    let cfg = config::Config::load(args.config.as_deref())?;

    info!(
        bridge = %cfg.bridge_name(),
        uplink = ?cfg.uplink(),
        "Starting OVS Port Agent"
    );

    match args.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            // For systemd-networkd, we don't need to ensure bridge topology here
            // The bridges are managed declaratively through the plugin system
            info!("Starting ovs-port-agent service...");

            // No need for NetworkManager configuration with systemd-networkd

            // Initialize FUSE mount base for Proxmox visibility
            if let Err(err) = convert_result(fuse::ensure_fuse_mount_base()) {
                warn!(error = %err, "Failed to ensure FUSE mount base");
            }

            // Clean up any existing mounts (safety cleanup)
            if let Err(err) = convert_result(fuse::cleanup_all_mounts()) {
                warn!(error = %err, "Failed to cleanup existing FUSE mounts");
            }

            // Initialize ledger
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path())).map_err(|e| {
                    crate::error::Error::Internal(format!("Failed to open ledger: {}", e))
                })?,
            ));

            // Initialize state manager
            let state_manager =
                std::sync::Arc::new(state::manager::StateManager::new(ledger.clone()));

            // Register plugins
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;

            // Set up streaming blockchain and footprint channel
            let (footprint_tx, footprint_rx) = tokio::sync::mpsc::unbounded_channel();
            let streaming_blockchain = Arc::new(
                streaming_blockchain::StreamingBlockchain::new("/var/lib/blockchain").await
                    .map_err(|e| error::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            );
            
            // Start footprint receiver
            let blockchain_clone = streaming_blockchain.clone();
            tokio::spawn(async move {
                blockchain_clone.start_footprint_receiver(footprint_rx).await
            });

            // Set up RPC state for container interface creation/removal
            let rpc_state = rpc::AppState {
                bridge: cfg.bridge_name().to_string(),
                ledger_path: cfg.ledger_path().to_string(),
                state_manager: Some(state_manager),
                streaming_blockchain,
                footprint_sender: footprint_tx,
            };

            info!("OVS Port Agent initialized successfully");
            info!("Container interface creation available via D-Bus API");
            info!(
                "Bridge: {} (uplink: {})",
                cfg.bridge_name(),
                cfg.uplink().unwrap_or("none")
            );

            // Run the RPC service - container interfaces will be created via D-Bus API calls
            convert_result(rpc::serve_with_state(rpc_state).await)?;
            Ok(())
        }
        Commands::Name { container, index } => {
            let name = naming::container_eth_name(&container, index);
            println!("{}", name);
            Ok(())
        }
        Commands::CreateInterface {
            raw_ifname,
            container_id,
            vmid,
        } => {
            info!(raw_ifname = %raw_ifname, container_id = %container_id, vmid = %vmid, "Creating container interface");

            let config = netlink::InterfaceConfig::new(
                cfg.bridge_name().to_string(),
                raw_ifname.clone(),
                container_id.clone(),
                vmid,
            )
            .with_interfaces_path(cfg.interfaces_path().to_string())
            .with_managed_tag(cfg.managed_block_tag().to_string())
            .with_enable_rename(cfg.enable_rename())
            .with_naming_template(cfg.naming_template().to_string())
            .with_ledger_path(cfg.ledger_path().to_string());

            convert_result(netlink::create_container_interface(config).await)?;

            info!(vmid = %vmid, "Container interface created successfully");
            println!("Container interface created successfully for VMID {}", vmid);
            Ok(())
        }
        Commands::RemoveInterface { interface_name } => {
            info!(interface_name = %interface_name, "Removing container interface");

            convert_result(
                netlink::remove_container_interface(
                    cfg.bridge_name().to_string(),
                    &interface_name,
                    cfg.interfaces_path().to_string(),
                    cfg.managed_block_tag().to_string(),
                    cfg.ledger_path().to_string(),
                )
                .await,
            )?;

            info!(interface_name = %interface_name, "Container interface removed successfully");
            println!(
                "Container interface {} removed successfully",
                interface_name
            );
            Ok(())
        }
        Commands::List => {
            info!("Listing container interfaces");
            let names = convert_result(nm_query::list_connection_names())?;
            for p in names.into_iter().filter(|n| n.starts_with("ovs-eth-")) {
                println!("{}", p.trim_start_matches("ovs-eth-"));
            }
            Ok(())
        }
        Commands::IntrospectSystemd => {
            Ok(())
        }
        Commands::ApplyState { state_file } => {
            info!(file = ?state_file, "Applying declarative state");

            // Initialize state manager
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path())).map_err(|e| {
                    crate::error::Error::Internal(format!("Failed to open ledger: {}", e))
                })?,
            ));
            let state_manager = state::manager::StateManager::new(ledger.clone());
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;

            // Load and apply state
            let desired_state =
                convert_result(state_manager.load_desired_state(&state_file).await)?;

            let report = convert_result(state_manager.apply_state(desired_state).await)?;

            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|_| "Failed to serialize report".to_string())
            );

            if report.success {
                info!("State applied successfully");
                Ok(())
            } else {
                Err(crate::error::Error::Internal(
                    "State apply failed".to_string(),
                ))
            }
        }
        Commands::QueryState { plugin } => {
            info!(plugin = ?plugin, "Querying current state");

            // Initialize state manager
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path())).map_err(|e| {
                    crate::error::Error::Internal(format!("Failed to open ledger: {}", e))
                })?,
            ));
            let state_manager = state::manager::StateManager::new(ledger.clone());
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;

            // Query state
            let state = if let Some(plugin_name) = plugin {
                convert_result(state_manager.query_plugin_state(&plugin_name).await)?
            } else {
                let current = convert_result(state_manager.query_current_state().await)?;
                serde_json::to_value(&current)
                    .map_err(|e| crate::error::Error::Internal(e.to_string()))?
            };

            println!(
                "{}",
                serde_json::to_string_pretty(&state)
                    .unwrap_or_else(|_| "Failed to serialize state".to_string())
            );
            Ok(())
        }
        Commands::ShowDiff { state_file } => {
            info!(file = ?state_file, "Calculating state diff");

            // Initialize state manager
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path())).map_err(|e| {
                    crate::error::Error::Internal(format!("Failed to open ledger: {}", e))
                })?,
            ));
            let state_manager = state::manager::StateManager::new(ledger.clone());
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;

            // Load desired state and calculate diff
            let desired_state =
                convert_result(state_manager.load_desired_state(&state_file).await)?;

            let diffs = convert_result(state_manager.show_diff(desired_state).await)?;

            println!(
                "{}",
                serde_json::to_string_pretty(&diffs)
                    .unwrap_or_else(|_| "Failed to serialize diffs".to_string())
            );
            Ok(())
        }
    }
}
