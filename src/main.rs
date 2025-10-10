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
mod nmcli_dyn;
mod rpc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{info, warn};
use std::path::PathBuf;

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
    /// Comprehensive NetworkManager introspection and debugging
    Introspect,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    logging::init_logging();

    let args = Cli::parse();
    let cfg = config::Config::load(args.config.as_deref())?;

    match args.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            // Ensure the bridge and optional uplink exist under NetworkManager control
            nm_bridge::ensure_bridge_topology(&cfg.bridge_name, cfg.uplink.as_deref(), 45)?;

            // Write NetworkManager unmanaged-devices config
            if !cfg.nm_unmanaged.is_empty() {
                if let Err(e) = nm_config::write_unmanaged_devices(&cfg.nm_unmanaged) {
                    warn!("failed to write NM unmanaged-devices config: {:?}", e);
                }
            }

            // Initialize FUSE mount base for Proxmox visibility
            if let Err(err) = fuse::ensure_fuse_mount_base() {
                warn!("failed to ensure FUSE mount base: {err:?}");
            }

            // Clean up any existing mounts (safety cleanup)
            if let Err(err) = fuse::cleanup_all_mounts() {
                warn!("failed to cleanup existing FUSE mounts: {err:?}");
            }

            // Set up RPC state for container interface creation/removal
            let rpc_state = rpc::AppState {
                bridge: cfg.bridge_name.clone(),
                ledger_path: cfg.ledger_path.clone(),
            };

            info!("OVS Port Agent initialized successfully");
            info!("Container interface creation available via D-Bus API");
            info!("Bridge: {} (uplink: {})", cfg.bridge_name, cfg.uplink.as_deref().unwrap_or("none"));

            // Run the RPC service - container interfaces will be created via D-Bus API calls
            rpc::serve_with_state(rpc_state).await?;
            Ok(())
        }
        Commands::Name { container, index } => {
            let name = naming::container_eth_name(&container, index);
            println!("{}", name);
            Ok(())
        }
        Commands::CreateInterface { raw_ifname, container_id, vmid } => {
            let bridge = cfg.bridge_name;
            let interfaces_path = cfg.interfaces_path;
            let managed_tag = cfg.managed_block_tag;
            let enable_rename = cfg.enable_rename;
            let naming_template = cfg.naming_template;
            let ledger_path = cfg.ledger_path;

            netlink::create_container_interface(
                bridge,
                &raw_ifname,
                &container_id,
                vmid,
                interfaces_path,
                managed_tag,
                enable_rename,
                naming_template,
                ledger_path,
            ).await?;
            println!("Container interface created successfully for VMID {}", vmid);
            Ok(())
        }
        Commands::RemoveInterface { interface_name } => {
            let bridge = cfg.bridge_name;
            let interfaces_path = cfg.interfaces_path;
            let managed_tag = cfg.managed_block_tag;
            let ledger_path = cfg.ledger_path;

            netlink::remove_container_interface(
                bridge,
                &interface_name,
                interfaces_path,
                managed_tag,
                ledger_path,
            ).await?;
            println!("Container interface {} removed successfully", interface_name);
            Ok(())
        }
        Commands::List => {
            let names = nmcli_dyn::list_connection_names()?;
            for p in names.into_iter().filter(|n| n.starts_with("dyn-eth-")) {
                println!("{}", p.trim_start_matches("dyn-eth-"));
            }
            Ok(())
        }
        Commands::Introspect => rpc::introspect_nm().await,
    }
}

