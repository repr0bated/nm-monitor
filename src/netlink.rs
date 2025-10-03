use crate::interfaces::update_interfaces_block;
use crate::ovs;
use crate::naming::render_template;
use crate::ledger::Ledger;
use crate::link;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::{collections::BTreeSet, path::PathBuf};
use tokio::time::{sleep, Duration, Instant};
use std::fs;

pub async fn monitor_links(
    bridge: String,
    include_prefixes: Vec<String>,
    interfaces_path: String,
    managed_tag: String,
    enable_rename: bool,
    naming_template: String,
    ledger_path: String,
) -> Result<()> {
    let interfaces_path = PathBuf::from(interfaces_path);

    // Try rtnetlink subscription via /proc/net/netlink as a simple presence check
    let have_netlink = fs::metadata("/proc/net/netlink").is_ok();
    let mut last_reconcile = Instant::now() - Duration::from_secs(3600);
    loop {
        // Cheap event hint: modification time change on /sys/class/net
        let tick = Instant::now();
        if let Err(err) = reconcile_once(
            &bridge,
            &include_prefixes,
            &interfaces_path,
            &managed_tag,
            enable_rename,
            &naming_template,
            &ledger_path,
        ) {
            warn!("reconcile failed: {err:?}");
        }
        last_reconcile = tick;

        // Fallback periodic sleep; when rtnetlink is added, we'll wake on events
        let period = if have_netlink { 1500 } else { 1000 };
        sleep(Duration::from_millis(period)).await;
    }
}

fn reconcile_once(
    bridge: &str,
    include_prefixes: &[String],
    interfaces_path: &PathBuf,
    managed_tag: &str,
    enable_rename: bool,
    naming_template: &str,
    ledger_path: &str,
) -> Result<()> {
    // Desired: all interfaces in /sys/class/net matching prefixes
    let desired_raw = list_sys_class_net(include_prefixes)?;
    // Optionally rename to template
    let mut desired = BTreeSet::new();
    for raw in desired_raw.iter() {
        let target = if enable_rename {
            // naive index=0 until container index is resolved
            render_template(naming_template, raw, 0)
        } else {
            raw.clone()
        };
        if enable_rename && *raw != target {
            if !link::exists(&target) {
                if let Err(e) = link::rename_safely(raw, &target) {
                    warn!("rename {raw} -> {target} failed: {e:?}");
                } else {
                    // ledger rename
                    let mut lg = Ledger::open(PathBuf::from(ledger_path))?;
                    let _ = lg.append(
                        "rename",
                        serde_json::json!({"old": raw, "new": target, "bridge": bridge}),
                    );
                }
            }
        }
        desired.insert(target);
    }

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
        let mut lg = Ledger::open(PathBuf::from(ledger_path))?;
        let _ = lg.append("ovs_add_port", serde_json::json!({"port": name, "bridge": bridge}));
    }
    for name in to_del.iter() {
        let _ = ovs::del_port(bridge, name).with_context(|| format!("deleting port {name}"))?;
        let mut lg = Ledger::open(PathBuf::from(ledger_path))?;
        let _ = lg.append("ovs_del_port", serde_json::json!({"port": name, "bridge": bridge}));
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
