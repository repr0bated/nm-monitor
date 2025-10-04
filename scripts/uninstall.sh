#!/usr/bin/env bash
set -euo pipefail

# Uninstall nm-monitor (ovs-port-agent) and clean up NetworkManager OVS profiles
# Usage: sudo ./scripts/uninstall.sh [--bridge ovsbr0] [--with-ovsbr1] [--purge-config] [--purge-ledger]

BRIDGE="ovsbr0"
WITH_OVSBR1=0
PURGE_CONFIG=0
PURGE_LEDGER=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge) BRIDGE="$2"; shift 2;;
    --with-ovsbr1) WITH_OVSBR1=1; shift;;
    --purge-config) PURGE_CONFIG=1; shift;;
    --purge-ledger) PURGE_LEDGER=1; shift;;
    *) echo "Unknown arg: $1"; exit 1;;
  esac
done

log() { echo "[uninstall] $*"; }

log "Stopping service"
systemctl disable --now ovs-port-agent 2>/dev/null || true

log "Removing systemd unit"
rm -f /etc/systemd/system/ovs-port-agent.service || true
systemctl daemon-reload || true

log "Removing binary"
rm -f /usr/local/bin/ovs-port-agent || true

nm_del_conn() {
  local name="$1"
  if nmcli -t -f NAME c show 2>/dev/null | grep -qx "$name"; then
    nmcli -w 5 c down "$name" >/dev/null 2>&1 || true
    nmcli c delete "$name" >/dev/null 2>&1 || true
    log "Removed NM connection: $name"
  fi
}

log "Removing dynamic NM connections (dyn-*)"
while IFS= read -r n; do
  nm_del_conn "$n"
  # remove matching dyn-port too
  pn="${n#dyn-eth-}"
  nm_del_conn "dyn-port-${pn}"
done < <(nmcli -t -f NAME,TYPE c show | awk -F: '/^dyn-eth-/ {print $1}')

remove_bridge_stack() {
  local br="$1"
  log "Removing NM connections for bridge: $br"
  # Known stack elements
  nm_del_conn "${br}-uplink-$(nmcli -t -f NAME c show | awk -F: -v b="$br" '$1 ~ "^"b"-uplink-" {print substr($1, index($1, b"-uplink-")+length(b"-uplink-")) ; exit }')" || true
  # Delete all matching by prefix (safe order)
  # ethernet slaves first
  while IFS= read -r n; do nm_del_conn "$n"; done < <(nmcli -t -f NAME,TYPE c show | awk -F: -v b="$br" '$1 ~ "^"b"-uplink-" && $2=="802-3-ethernet" {print $1}')
  # ovs-port (uplinks + internal)
  while IFS= read -r n; do nm_del_conn "$n"; done < <(nmcli -t -f NAME,TYPE c show | awk -F: -v b="$br" '$1 ~ "^"b"-port-" && $2=="ovs-port" {print $1}')
  # ovs-interface (bridge L3)
  nm_del_conn "${br}-if"
  # bridge itself (last)
  nm_del_conn "$br"
}

remove_bridge_stack "$BRIDGE"

if [[ "$WITH_OVSBR1" == 1 ]]; then
  remove_bridge_stack "ovsbr1"
fi

if [[ "$PURGE_CONFIG" == 1 ]]; then
  log "Removing config /etc/ovs-port-agent/config.toml"
  rm -f /etc/ovs-port-agent/config.toml || true
fi

if [[ "$PURGE_LEDGER" == 1 ]]; then
  log "Removing ledger /var/lib/ovs-port-agent/ledger.jsonl"
  rm -f /var/lib/ovs-port-agent/ledger.jsonl || true
fi

log "Done."