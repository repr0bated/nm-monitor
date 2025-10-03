mod config;
mod logging;
mod naming;
mod ovs;
mod interfaces;
mod netlink;
mod rpc;
mod ledger;
mod link;

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{error, info};
use std::path::PathBuf;
use tokio::signal;

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
    /// List OVS ports on the configured bridge
    List,
    /// D-Bus introspection: print NM root interfaces
    Introspect,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    logging::init_logging();

    let args = Cli::parse();
    let cfg = config::Config::load(args.config.as_deref())?;

    match args.command.unwrap_or(Commands::Run) {
        Commands::Run => run_agent(cfg).await,
        Commands::Name { container, index } => {
            let name = naming::container_eth_name(&container, index);
            println!("{}", name);
            Ok(())
        }
        Commands::List => {
            let ports = ovs::list_ports(&cfg.bridge_name)?;
            for p in ports { println!("{}", p); }
            Ok(())
        }
        Commands::Introspect => {
            rpc::introspect_nm().await
        }
    }
}

async fn run_agent(cfg: config::Config) -> Result<()> {
    info!("starting ovs-port-agent on bridge {}", cfg.bridge_name);

    // Start D-Bus service (best-effort)
    let state = rpc::AppState { bridge: cfg.bridge_name.clone(), ledger_path: cfg.ledger_path.clone() };
    let _rpc_handle = tokio::spawn(rpc::serve_with_state(state));

    // Start link monitor (best-effort). For now, periodic reconcile.
    let bridge = cfg.bridge_name.clone();
    let include_prefixes = cfg.include_prefixes.clone();
    let interfaces_path = cfg.interfaces_path.clone();
    let managed_tag = cfg.managed_block_tag.clone();
    let enable_rename = cfg.enable_rename;
    let naming_template = cfg.naming_template.clone();
    let ledger_path = cfg.ledger_path.clone();

    let monitor_handle = tokio::spawn(async move {
        if let Err(err) = netlink::monitor_links(
            bridge,
            include_prefixes,
            interfaces_path,
            managed_tag,
            enable_rename,
            naming_template,
            ledger_path,
        ).await {
            error!("link monitor exited with error: {err:?}");
        }
    });

    signal::ctrl_c().await?;
    info!("shutdown requested, exiting");
    monitor_handle.abort();
    let _ = monitor_handle.await;
    Ok(())
}
