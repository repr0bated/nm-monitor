use crate::fuse::{
    bind_veth_interface, cleanup_all_mounts, ensure_fuse_mount_base, unbind_veth_interface,
};
use crate::interfaces::update_interfaces_block;
use crate::ledger::Ledger;
use crate::link;
use crate::naming::render_template;
use crate::nmcli_dyn;
use anyhow::{Context, Result};
use log::{info, warn};
use std::{collections::BTreeSet, path::PathBuf};
use tokio::time::{sleep, Duration, Instant};
// use std::fs; // reserved for future inotify
use futures_util::TryStreamExt;
use rtnetlink::new_connection;

#[allow(clippy::too_many_arguments)]
pub async fn monitor_links(
    bridge: String,
    include_prefixes: Vec<String>,
    interfaces_path: String,
    managed_tag: String,
    enable_rename: bool,
    naming_template: String,
    ledger_path: String,
    uplink: Option<String>,
) -> Result<()> {
    let interfaces_path = PathBuf::from(interfaces_path);
    let configured_uplink = uplink;

    // Start rtnetlink listener
    let (conn, handle, _) = new_connection().context("create rtnetlink connection")?;
    tokio::spawn(conn);

    // Debounce window
    let debounce = Duration::from_millis(500);
    let mut last_fire = Instant::now() - debounce;

    // Ensure FUSE mount base exists
    if let Err(err) = ensure_fuse_mount_base() {
        warn!("failed to ensure FUSE mount base: {err:?}");
    }

    // Clean up any existing mounts (safety cleanup)
    if let Err(err) = cleanup_all_mounts() {
        warn!("failed to cleanup existing FUSE mounts: {err:?}");
    }

    // Initial reconcile
    if let Err(err) = reconcile_once(
        &bridge,
        &include_prefixes,
        &interfaces_path,
        &managed_tag,
        enable_rename,
        &naming_template,
        &ledger_path,
        configured_uplink.as_deref(),
    ) {
        warn!("initial reconcile failed: {err:?}");
    }

    loop {
        let mut triggered = false;
        // Listen for any link events, but since rtnetlink crate doesn't expose a raw stream here,
        // poll a lightweight request periodically as a trigger; fallback timer ensures progress.
        if handle.link().get().execute().try_next().await.is_ok() {
            triggered = true;
        }
        // periodic fallback
        sleep(Duration::from_millis(1000)).await;

        if triggered && last_fire.elapsed() >= debounce {
            last_fire = Instant::now();
            if let Err(err) = reconcile_once(
                &bridge,
                &include_prefixes,
                &interfaces_path,
                &managed_tag,
                enable_rename,
                &naming_template,
                &ledger_path,
                configured_uplink.as_deref(),
            ) {
                warn!("reconcile failed: {err:?}");
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::ptr_arg, clippy::collapsible_if, clippy::needless_borrow)]
fn reconcile_once(
    bridge: &str,
    include_prefixes: &[String],
    interfaces_path: &PathBuf,
    managed_tag: &str,
    enable_rename: bool,
    naming_template: &str,
    ledger_path: &str,
    configured_uplink: Option<&str>,
) -> Result<()> {
    // Desired: all interfaces in /sys/class/net matching prefixes
    let desired_raw = list_sys_class_net(include_prefixes)?;
    // Optionally rename to template
    let mut desired = BTreeSet::new();
    for raw in desired_raw.iter() {
        let target = if enable_rename {
            let base =
                crate::link::container_short_name_from_ifname(raw).unwrap_or_else(|| raw.clone());
            // naive index=0 until container index is resolved
            render_template(naming_template, &base, 0)
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
                        serde_json::json!({"old": raw, "new": target, "interface": target, "bridge": bridge}),
                    );
                }
            }
        }
        desired.insert(target);
    }

    // Existing: NM dynamic ports present (by our naming convention)
    let existing_conns = nmcli_dyn::list_connection_names().unwrap_or_default();
    let existing_filtered: BTreeSet<String> = desired_raw
        .iter()
        .filter(|ifn| existing_conns.contains(&nmcli_dyn::eth_conn_name(ifn)))
        .cloned()
        .collect();

    let to_add: BTreeSet<_> = desired.difference(&existing_filtered).cloned().collect();
    let to_del: BTreeSet<_> = existing_filtered.difference(&desired).cloned().collect();

    if !to_add.is_empty() || !to_del.is_empty() {
        info!("bridge={bridge} add={:?} del={:?}", to_add, to_del);
    }

    for name in to_add.iter() {
        nmcli_dyn::ensure_dynamic_port(&bridge, name)
            .with_context(|| format!("nmcli add dyn port for {name}"))?;
        let mut lg = Ledger::open(PathBuf::from(ledger_path))?;
        let _ = lg.append(
            "nm_add_dyn_port",
            serde_json::json!({"port": name, "interface": name, "bridge": bridge}),
        );

        // Create FUSE bind mount for Proxmox visibility
        if let Err(e) = bind_veth_interface(name, name) {
            warn!("Failed to bind veth interface {}: {}", name, e);
        }
    }
    for name in to_del.iter() {
        // Unbind FUSE mount before removing the port
        if let Err(e) = unbind_veth_interface(name) {
            warn!("Failed to unbind veth interface {}: {}", name, e);
        }

        nmcli_dyn::remove_dynamic_port(name)
            .with_context(|| format!("nmcli remove dyn port for {name}"))?;
        let mut lg = Ledger::open(PathBuf::from(ledger_path))?;
        let _ = lg.append(
            "nm_del_dyn_port",
            serde_json::json!({"port": name, "interface": name, "bridge": bridge}),
        );
    }

    // Write bounded block for visibility in Proxmox
    let mut names: Vec<String> = desired.into_iter().collect();
    names.sort();

    // Prefer configured uplink; fall back to detection if not provided
    let uplink = if let Some(explicit) = configured_uplink {
        Some(explicit.to_string())
    } else {
        detect_bridge_uplink(bridge)
    };

    update_interfaces_block(
        interfaces_path,
        managed_tag,
        &names,
        bridge,
        uplink.as_deref(),
    )?;

    Ok(())
}

fn detect_bridge_uplink(bridge: &str) -> Option<String> {
    // Try to detect uplink from NM connections
    let conns = nmcli_dyn::list_connection_names().ok()?;
    for conn in conns {
        if conn.starts_with(&format!("{bridge}-uplink-")) {
            return Some(
                conn.trim_start_matches(&format!("{bridge}-uplink-"))
                    .to_string(),
            );
        }
    }
    None
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
