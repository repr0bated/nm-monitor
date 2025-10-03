use crate::interfaces::update_interfaces_block;
use crate::ovs;
use crate::naming::render_template;
use anyhow::{Context, Result};
use log::{info, warn};
use std::{collections::BTreeSet, path::PathBuf};
use tokio::time::{sleep, Duration};

pub async fn monitor_links(
    bridge: String,
    include_prefixes: Vec<String>,
    interfaces_path: String,
    managed_tag: String,
) -> Result<()> {
    let interfaces_path = PathBuf::from(interfaces_path);

    loop {
        if let Err(err) = reconcile_once(&bridge, &include_prefixes, &interfaces_path, &managed_tag) {
            warn!("reconcile failed: {err:?}");
        }
        // TODO: replace with inotify/netlink subscription; for now, periodic scan
        sleep(Duration::from_millis(1000)).await;
    }
}

fn reconcile_once(
    bridge: &str,
    include_prefixes: &[String],
    interfaces_path: &PathBuf,
    managed_tag: &str,
) -> Result<()> {
    // Desired: all interfaces in /sys/class/net matching prefixes
    let desired_raw = list_sys_class_net(include_prefixes)?;
    // Future: rename to template, track mapping. For now, use raw names.
    let desired = desired_raw;

    // Existing: OVS ports on the bridge matching prefixes
    let existing = ovs::list_ports(bridge).unwrap_or_default();
    let existing_filtered: BTreeSet<String> = existing
        .into_iter()
        .filter(|n| include_prefixes.iter().any(|p| n.starts_with(p)))
        .collect();

    let to_add: BTreeSet<_> = desired.difference(&existing_filtered).cloned().collect();
    let to_del: BTreeSet<_> = existing_filtered.difference(&desired).cloned().collect();

    if !to_add.is_empty() || !to_del.is_empty() {
        info!("bridge={bridge} add={:?} del={:?}", to_add, to_del);
    }

    for name in to_add.iter() {
        let _ = ovs::add_port(bridge, name).with_context(|| format!("adding port {name}"))?;
    }
    for name in to_del.iter() {
        let _ = ovs::del_port(bridge, name).with_context(|| format!("deleting port {name}"))?;
    }

    // Write bounded block for visibility in Proxmox
    let mut names: Vec<String> = desired.into_iter().collect();
    names.sort();
    update_interfaces_block(interfaces_path, managed_tag, &names, bridge)?;

    Ok(())
}

fn list_sys_class_net(include_prefixes: &[String]) -> Result<BTreeSet<String>> {
    let mut set = BTreeSet::new();
    let dir = std::fs::read_dir("/sys/class/net").context("reading /sys/class/net")?;
    for entry in dir.flatten() {
        if let Ok(name) = entry.file_name().into_string() {
            if name == "lo" || name.starts_with("ovs-system") {
                continue;
            }
            if include_prefixes.iter().any(|p| name.starts_with(p)) {
                set.insert(name);
            }
        }
    }
    Ok(set)
}
