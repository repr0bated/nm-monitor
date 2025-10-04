#!/usr/bin/env bash
# Diagnose OVS bridge activation issues

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_check() { echo -e "${BLUE}[CHECK]${NC} $*"; }

# Default bridge
: "${BRIDGE:=ovsbr0}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --bridge) BRIDGE="$2"; shift 2 ;;
        --help)
            echo "Usage: $0 [--bridge BRIDGE]"
            echo "Diagnose OVS bridge activation issues"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo "=== OVS Bridge Diagnostics for $BRIDGE ==="
echo

# Check services
log_check "Service Status"
echo -n "NetworkManager: "
systemctl is-active NetworkManager || echo "STOPPED"
echo -n "Open vSwitch: "
systemctl is-active openvswitch-switch 2>/dev/null || systemctl is-active openvswitch 2>/dev/null || echo "STOPPED"
echo

# Check OVS database
log_check "OVS Database"
if command -v ovs-vsctl >/dev/null 2>&1; then
    echo "OVS bridges:"
    ovs-vsctl list-br 2>/dev/null || echo "  ERROR: Cannot list bridges"
    
    if ovs-vsctl br-exists "$BRIDGE" 2>/dev/null; then
        echo
        echo "Bridge $BRIDGE details:"
        ovs-vsctl show | grep -A 10 "Bridge.*$BRIDGE" || echo "  ERROR: Cannot show bridge"
    else
        log_warn "Bridge $BRIDGE does not exist in OVS"
    fi
else
    log_error "ovs-vsctl not found"
fi
echo

# Check NetworkManager connections
log_check "NetworkManager Connections"
echo "All OVS-related connections:"
nmcli -t -f NAME,TYPE,STATE,DEVICE connection show | grep -E "(ovs-|${BRIDGE})" || echo "  None found"
echo

# Check specific bridge connections
log_check "Bridge Connection Details"
if nmcli connection show "$BRIDGE" >/dev/null 2>&1; then
    echo "Bridge connection $BRIDGE:"
    nmcli connection show "$BRIDGE" | grep -E "(GENERAL|connection\.|ovs-bridge\.)" | head -20
else
    log_error "No connection named $BRIDGE"
fi
echo

# Check for port connections
log_check "Port Connections"
local port_conns=$(nmcli -t -f NAME,TYPE connection show | grep "^${BRIDGE}.*:ovs-port" | cut -d: -f1)
if [[ -n "$port_conns" ]]; then
    echo "Found port connections:"
    echo "$port_conns" | while IFS= read -r port; do
        echo "  - $port"
        nmcli -t -f connection.master,GENERAL.STATE connection show "$port" 2>/dev/null | sed 's/^/    /'
    done
else
    log_warn "No port connections found for $BRIDGE"
fi
echo

# Check for interface connections
log_check "Interface Connections"
local if_conns=$(nmcli -t -f NAME,TYPE connection show | grep "^${BRIDGE}.*:ovs-interface" | cut -d: -f1)
if [[ -n "$if_conns" ]]; then
    echo "Found interface connections:"
    echo "$if_conns" | while IFS= read -r iface; do
        echo "  - $iface"
        nmcli -t -f connection.master,GENERAL.STATE,ipv4.addresses connection show "$iface" 2>/dev/null | sed 's/^/    /'
    done
else
    log_warn "No interface connections found for $BRIDGE"
fi
echo

# Check system logs
log_check "Recent NetworkManager Logs"
journalctl -u NetworkManager -n 30 --no-pager | grep -E "(${BRIDGE}|ovs|error|fail)" -i | tail -10 || echo "  No relevant logs found"
echo

# Check network interfaces
log_check "Network Interfaces"
echo "OVS-related interfaces:"
ip link show | grep -E "(${BRIDGE}|ovs)" || echo "  None found"
echo

# Check for common issues
log_check "Common Issues"

# Issue 1: Missing OVS kernel module
if ! lsmod | grep -q openvswitch; then
    log_warn "OVS kernel module not loaded"
    echo "  Fix: modprobe openvswitch"
else
    log_info "OVS kernel module loaded"
fi

# Issue 2: OVS database issues
if command -v ovsdb-client >/dev/null 2>&1; then
    if ! ovsdb-client list-dbs 2>/dev/null | grep -q Open_vSwitch; then
        log_error "OVS database not accessible"
    else
        log_info "OVS database accessible"
    fi
fi

# Issue 3: NetworkManager OVS plugin
if nmcli -t -f PLUGIN connection show 2>&1 | grep -q "ovs"; then
    log_info "NetworkManager OVS plugin available"
else
    log_warn "NetworkManager OVS plugin might not be installed"
    echo "  Fix: apt install network-manager-openvswitch (or equivalent)"
fi

# Issue 4: Conflicting connections
local conflicts=$(nmcli -t -f NAME,DEVICE connection show --active | grep -E ":(${BRIDGE}|ens[0-9]+|eth[0-9]+)$" | grep -v ovs)
if [[ -n "$conflicts" ]]; then
    log_warn "Found potentially conflicting active connections:"
    echo "$conflicts" | sed 's/^/  /'
fi

echo
log_info "Diagnostics complete"

# Provide recommendations
echo
echo "=== Recommendations ==="

if ! systemctl is-active --quiet NetworkManager; then
    echo "1. Start NetworkManager: systemctl start NetworkManager"
fi

if ! systemctl is-active --quiet openvswitch-switch 2>/dev/null && ! systemctl is-active --quiet openvswitch 2>/dev/null; then
    echo "2. Start Open vSwitch: systemctl start openvswitch-switch"
fi

if ! ovs-vsctl br-exists "$BRIDGE" 2>/dev/null; then
    echo "3. Add bridge to OVS: ovs-vsctl add-br $BRIDGE"
fi

echo "4. To manually activate: nmcli connection up $BRIDGE"
echo "5. For more details: journalctl -u NetworkManager -f"