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
mod link;
mod logging;
mod naming;
mod netlink;
mod ovsdb_dbus;
mod rpc;
mod services;
mod state;
mod systemd_dbus;
mod zbus_networkd;

use crate::error::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "ovs-port-agent", version, about = "OVS container port agent", long_about=None)]
struct Cli {
    /// Path to config file is (default: /etc/ovs-port-agent/config.json)
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
    /// Create OVS bridge via OVSDB D-Bus
    CreateBridge {
        /// Bridge name
        bridge_name: String,
    },
    /// Delete OVS bridge via OVSDB D-Bus
    DeleteBridge {
        /// Bridge name
        bridge_name: String,
    },
    /// Add port to OVS bridge via OVSDB D-Bus
    AddPort {
        /// Bridge name
        bridge_name: String,
        /// Port/interface name
        port_name: String,
    },
    /// Comprehensive systemd-networkd introspection and debugging
    IntrospectSystemd,
    /// Apply declarative state from JSON file
    ApplyState {
        /// Path to state JSON file
        state_file: std::path::PathBuf,
    },
    /// Query current system state
    QueryState {
        /// Optional plugin name (network, filesystem, etc.)
        plugin: Option<String>,
    },
    /// Show diff between current and desired state
    ShowDiff {
        /// Path to desired state JSON file
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

            // Initialize state manager (ledger functionality moved to streaming blockchain)
            let state_manager =
                std::sync::Arc::new(state::manager::StateManager::new());

            // Register plugins
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::DockerStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetmakerStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::DockerStatePlugin::new()))
                .await;

            // Set up streaming blockchain and footprint channel
            let (footprint_tx, footprint_rx) = tokio::sync::mpsc::unbounded_channel();
            let streaming_blockchain = Arc::new(
                streaming_blockchain::StreamingBlockchain::new("/var/lib/blockchain").await
                    .map_err(|e| error::Error::Io(std::io::Error::other(e)))?
            );
            
            // Start footprint receiver
            let blockchain_clone = streaming_blockchain.clone();
            tokio::spawn(async move {
                blockchain_clone.start_footprint_receiver(footprint_rx).await
            });

            // Set up RPC state for container interface creation/removal
            let rpc_state = rpc::AppState {
                bridge: cfg.bridge_name().to_string(),
                ledger_path: cfg.ledger_path().to_string(), // Keep for compatibility, but not used
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
            .with_ledger_path(cfg.ledger_path().to_string()); // Ledger path kept for compatibility

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
            info!("Listing OVS bridge ports");
            // Use OVSDB D-Bus to list ports on the bridge
            let client = ovsdb_dbus::OvsdbClient::new().await
                .map_err(|e| error::Error::Internal(format!("Failed to connect to OVSDB: {}", e)))?;
            
            let ports = client.list_bridge_ports(cfg.bridge_name()).await
                .map_err(|e| error::Error::Internal(format!("Failed to list ports: {}", e)))?;
            
            for port in ports {
                println!("{}", port);
            }
            Ok(())
        }
        Commands::CreateBridge { bridge_name } => {
            info!(bridge = %bridge_name, "Creating OVS bridge via OVSDB D-Bus");
            let client = ovsdb_dbus::OvsdbClient::new().await
                .map_err(|e| error::Error::Internal(format!("Failed to connect to OVSDB: {}", e)))?;
            
            client.create_bridge(&bridge_name).await
                .map_err(|e| error::Error::Internal(format!("Failed to create bridge: {}", e)))?;
            
            info!(bridge = %bridge_name, "Bridge created successfully");
            println!("Bridge {} created successfully", bridge_name);
            Ok(())
        }
        Commands::DeleteBridge { bridge_name } => {
            info!(bridge = %bridge_name, "Deleting OVS bridge via OVSDB D-Bus");
            let client = ovsdb_dbus::OvsdbClient::new().await
                .map_err(|e| error::Error::Internal(format!("Failed to connect to OVSDB: {}", e)))?;
            
            client.delete_bridge(&bridge_name).await
                .map_err(|e| error::Error::Internal(format!("Failed to delete bridge: {}", e)))?;
            
            info!(bridge = %bridge_name, "Bridge deleted successfully");
            println!("Bridge {} deleted successfully", bridge_name);
            Ok(())
        }
        Commands::AddPort { bridge_name, port_name } => {
            info!(bridge = %bridge_name, port = %port_name, "Adding port via OVSDB D-Bus");
            let client = ovsdb_dbus::OvsdbClient::new().await
                .map_err(|e| error::Error::Internal(format!("Failed to connect to OVSDB: {}", e)))?;

            // Check if bridge exists before attempting to add port
            let bridge_exists = client.bridge_exists(&bridge_name).await
                .map_err(|e| error::Error::Internal(format!("Failed to check bridge existence: {}", e)))?;

            if !bridge_exists {
                return Err(error::Error::Internal(format!(
                    "Bridge '{}' does not exist. Create the bridge first with: ovs-port-agent create-bridge {}",
                    bridge_name, bridge_name
                )));
            }

            client.add_port(&bridge_name, &port_name).await
                .map_err(|e| error::Error::Internal(format!("Failed to add port: {}", e)))?;

            info!(bridge = %bridge_name, port = %port_name, "Port added successfully");
            println!("Port {} added to bridge {} successfully", port_name, bridge_name);
            Ok(())
        }
        Commands::IntrospectSystemd => {
            Ok(())
        }
        Commands::ApplyState { state_file } => {
            info!(file = ?state_file, "Applying declarative state");

            // Initialize state manager (ledger functionality moved to streaming blockchain)
            let state_manager = state::manager::StateManager::new();
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::DockerStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetmakerStatePlugin::new()))
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

            // Initialize state manager (ledger functionality moved to streaming blockchain)
            let state_manager = state::manager::StateManager::new();
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::DockerStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetmakerStatePlugin::new()))
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

            // Initialize state manager (ledger functionality moved to streaming blockchain)
            let state_manager = state::manager::StateManager::new();
            state_manager
                .register_plugin(Box::new(state::plugins::NetStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetcfgStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::DockerStatePlugin::new()))
                .await;
            state_manager
                .register_plugin(Box::new(state::plugins::NetmakerStatePlugin::new()))
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
