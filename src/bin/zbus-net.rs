use anyhow::Result;
use clap::{Parser, Subcommand};

// Pull shared module from src/ for this standalone bin
#[path = "../zbus_networkd.rs"]
mod zbus_networkd;
#[path = "../ovs_introspect.rs"]
mod ovs_introspect;

#[derive(Parser)]
#[command(name = "zbus-net", about = "systemd-networkd zbus helper")] 
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Reload,
    List,
    Status { ifname: String },
    Reconfig { ifname: String },
    OvsBridge { name: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = zbus_networkd::NetworkdZbus::new().await?;

    match cli.cmd {
        Command::Reload => {
            client.reload().await?;
            println!("Reload invoked");
        }
        Command::List => {
            for l in client.list_links().await? {
                println!("{}\t{}\t{}", l.index, l.name, l.operational_state);
            }
        }
        Command::Status { ifname } => {
            let idx = client.ifindex_by_name(&ifname).await?;
            let d = client.link_detail(idx).await?;
            println!(
                "{} (idx {}): op={} admin={} addr={}",
                d.name, d.index, d.operational_state, d.administrative_state, d.address_state
            );
        }
        Command::Reconfig { ifname } => {
            client.reconfigure_link(&ifname).await?;
            println!("Reconfigure requested for {}", ifname);
        }
        Command::OvsBridge { name } => {
            let v = ovs_introspect::bridge_info_json(&name)?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    }

    Ok(())
}
