//! OVS Port Agent - Main Application

mod command;
mod error;
mod ovs_flows;
use crate::ovs_flows::OvsFlowManager;
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

use crate::error::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "ovs-port-agent", version, about = "OVS container port agent", long_about=None)]
struct Cli {
    /// Path to config file (default: /etc/ovs-port-agent/config.toml)
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
            // Ensure the bridge and optional uplink exist under NetworkManager control
            convert_result(nm_bridge::ensure_bridge_topology(
                cfg.bridge_name(),
                cfg.uplink(),
                45,
            ))?;

            // Write NetworkManager unmanaged-devices config
            if !cfg.nm_unmanaged().is_empty() {
                if let Err(e) = nm_config::write_unmanaged_devices(cfg.nm_unmanaged()) {
                    warn!(error = %e, "Failed to write NM unmanaged-devices config");
                }
            }

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
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path()))
                    .map_err(|e| crate::error::Error::Internal(format!("Failed to open ledger: {}", e)))?
            ));

            // Initialize state manager
            let state_manager = std::sync::Arc::new(state::manager::StateManager::new(ledger.clone()));
            
            // Register network plugin
            state_manager.register_plugin(Box::new(state::plugins::NetworkStatePlugin::new())).await;

            // Set up RPC state for container interface creation/removal
            let rpc_state = rpc::AppState {
                bridge: cfg.bridge_name().to_string(),
                ledger_path: cfg.ledger_path().to_string(),
                flow_manager: OvsFlowManager::new(cfg.bridge_name().to_string()),
                state_manager: Some(state_manager),
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
            info!("Running systemd-networkd introspection");
            convert_result(rpc::introspect_systemd_networkd().await)
        }
        Commands::ApplyState { state_file } => {
            info!(file = ?state_file, "Applying declarative state");
            
            // Initialize state manager
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path()))
                    .map_err(|e| crate::error::Error::Internal(format!("Failed to open ledger: {}", e)))?
            ));
            let state_manager = state::manager::StateManager::new(ledger.clone());
            state_manager.register_plugin(Box::new(state::plugins::NetworkStatePlugin::new())).await;

            // Load and apply state
            let desired_state = convert_result(
                state_manager.load_desired_state(&state_file).await
            )?;
            
            let report = convert_result(
                state_manager.apply_state(desired_state).await
            )?;

            println!("{}", serde_json::to_string_pretty(&report)
                .unwrap_or_else(|_| "Failed to serialize report".to_string()));
            
            if report.success {
                info!("State applied successfully");
                Ok(())
            } else {
                Err(crate::error::Error::Internal("State apply failed".to_string()))
            }
        }
        Commands::QueryState { plugin } => {
            info!(plugin = ?plugin, "Querying current state");
            
            // Initialize state manager
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path()))
                    .map_err(|e| crate::error::Error::Internal(format!("Failed to open ledger: {}", e)))?
            ));
            let state_manager = state::manager::StateManager::new(ledger.clone());
            state_manager.register_plugin(Box::new(state::plugins::NetworkStatePlugin::new())).await;

            // Query state
            let state = if let Some(plugin_name) = plugin {
                convert_result(
                    state_manager.query_plugin_state(&plugin_name).await
                )?
            } else {
                let current = convert_result(
                    state_manager.query_current_state().await
                )?;
                serde_json::to_value(&current)
                    .map_err(|e| crate::error::Error::Internal(e.to_string()))?
            };

            println!("{}", serde_json::to_string_pretty(&state)
                .unwrap_or_else(|_| "Failed to serialize state".to_string()));
            Ok(())
        }
        Commands::ShowDiff { state_file } => {
            info!(file = ?state_file, "Calculating state diff");
            
            // Initialize state manager
            let ledger = std::sync::Arc::new(tokio::sync::Mutex::new(
                ledger::Ledger::open(std::path::PathBuf::from(cfg.ledger_path()))
                    .map_err(|e| crate::error::Error::Internal(format!("Failed to open ledger: {}", e)))?
            ));
            let state_manager = state::manager::StateManager::new(ledger.clone());
            state_manager.register_plugin(Box::new(state::plugins::NetworkStatePlugin::new())).await;

            // Load desired state and calculate diff
            let desired_state = convert_result(
                state_manager.load_desired_state(&state_file).await
            )?;
            
            let diffs = convert_result(
                state_manager.show_diff(desired_state).await
            )?;

            println!("{}", serde_json::to_string_pretty(&diffs)
                .unwrap_or_else(|_| "Failed to serialize diffs".to_string()));
            Ok(())
        }
    }
}
