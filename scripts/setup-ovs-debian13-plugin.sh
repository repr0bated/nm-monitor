#!/usr/bin/env bash
# OVS Bridge Setup for Debian 13
## Uses gdbus to create bridges (accurate state), then plugin system for management

set -euo pipefail

# Configuration
readonly SCRIPT_NAME="$(basename "$0")"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(dirname "$SCRIPT_DIR")"
readonly CONFIG_FILE="${REPO_ROOT}/config/examples/debian13-ovs-bridges.yaml"

# Colors
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly RED='\033[0;31m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

# Functions
log() {
    echo -e "$1" >&2
}

error_exit() {
    log "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    error_exit "This script must be run as root"
fi

log "${BLUE}========================================${NC}"
log "${BLUE} OVS Bridge Setup for Debian 13${NC}"
log "${BLUE} Phase 1: gdbus bridge creation${NC}"
log "${BLUE} Phase 2: Plugin state management${NC}"
log "${BLUE}========================================${NC}"
echo ""

# Detect primary interface
log "${YELLOW}[INFO]${NC} Detecting primary network interface..."
PRIMARY_IFACE=$(ip -o -4 route show default | awk '$2 !~ /^(lo|ovsbr|docker|br-|veth)/ {print $5}' | head -1)

if [[ -z "$PRIMARY_IFACE" ]]; then
    PRIMARY_IFACE=$(ip -o -4 addr show | awk '$2 !~ /^(lo|ovsbr|docker|br-|veth)/ {print $2}' | head -1)
fi

if [[ -z "$PRIMARY_IFACE" ]]; then
    error_exit "Could not detect primary network interface"
fi

log "${GREEN}[INFO]${NC} Detected primary interface: $PRIMARY_IFACE"
echo ""

# Show current network state
log "${BLUE}[INFO]${NC} Current network state:"
echo ""
ip -brief addr show
echo ""

# Show configuration
log "${BLUE}[INFO]${NC} Bridges to create via gdbus:"
echo ""
log "  ${GREEN}ovsbr0:${NC}"
log "    - IP: 80.209.240.244/24"
log "    - Gateway: 80.209.240.129"
log "    - DNS: 8.8.8.8, 8.8.4.4"
log "    - Port: $PRIMARY_IFACE"
echo ""
log "  ${GREEN}ovsbr1:${NC}"
log "    - IP: 80.209.242.196/25"
log "    - Gateway: 80.209.242.129"
log "    - DNS: 8.8.8.8, 8.8.4.4"
log "    - Ports: none (isolated)"
echo ""

# Ask for confirmation
read -p "Create bridges with gdbus then manage with plugins? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log "${YELLOW}[INFO]${NC} Operation cancelled"
    exit 0
fi

# Helper function to convert IP to uint32 for D-Bus (network byte order)
ip_to_uint32() {
    local ip=$1
    local a b c d
    IFS=. read -r a b c d <<< "$ip"
    echo $((a + (b * 256) + (c * 256 * 256) + (d * 256 * 256 * 256)))
}

DNS1=$(ip_to_uint32 "8.8.8.8")
DNS2=$(ip_to_uint32 "8.8.4.4")

log ""
log "${BLUE}========================================${NC}"
log "${BLUE} PHASE 1: Create bridges with gdbus${NC}"
log "${BLUE}========================================${NC}"
log ""

# Create ovsbr0 bridge
log "${YELLOW}[INFO]${NC} Creating ovsbr0 bridge via gdbus..."
BRIDGE0_RESULT=$(gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager/Settings \
  --method org.freedesktop.NetworkManager.Settings.AddConnection2 \
  "{'connection': {'id': <'ovsbr0'>, 'type': <'ovs-bridge'>, 'interface-name': <'ovsbr0'>, 'autoconnect': <true>}, 'ovs-bridge': {'stp-enable': <false>, 'mcast-snooping-enable': <true>}, 'ipv4': {'method': <'manual'>, 'address-data': <[{'address': '80.209.240.244', 'prefix': <uint32 24>}]>, 'gateway': <'80.209.240.129'>, 'dns': <[uint32 $DNS1, uint32 $DNS2]>}, 'ipv6': {'method': <'disabled'>}}" \
  1 \
  {} 2>&1) || error_exit "Failed to create ovsbr0: $BRIDGE0_RESULT"

log "${GREEN}[SUCCESS]${NC} ovsbr0 created"
echo "$BRIDGE0_RESULT"
echo ""

# Create ovsbr0 port (connects bridge to port)
log "${YELLOW}[INFO]${NC} Creating ovsbr0 port..."
PORT0_RESULT=$(gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager/Settings \
  --method org.freedesktop.NetworkManager.Settings.AddConnection2 \
  "{'connection': {'id': <'ovsbr0-port'>, 'type': <'ovs-port'>, 'interface-name': <'ovsbr0-port'>, 'master': <'ovsbr0'>, 'slave-type': <'ovs-bridge'>, 'autoconnect': <true>}, 'ovs-port': {}}" \
  1 \
  {} 2>&1) || error_exit "Failed to create ovsbr0 port: $PORT0_RESULT"

log "${GREEN}[SUCCESS]${NC} ovsbr0-port created"
echo "$PORT0_RESULT"
echo ""

# Create ovsbr0 interface (attaches physical interface)
log "${YELLOW}[INFO]${NC} Creating ovsbr0 interface for $PRIMARY_IFACE..."
IFACE0_RESULT=$(gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager/Settings \
  --method org.freedesktop.NetworkManager.Settings.AddConnection2 \
  "{'connection': {'id': <'ovsbr0-$PRIMARY_IFACE'>, 'type': <'ovs-interface'>, 'interface-name': <'$PRIMARY_IFACE'>, 'master': <'ovsbr0-port'>, 'slave-type': <'ovs-port'>, 'autoconnect': <true>}, 'ovs-interface': {'type': <'internal'>}}" \
  1 \
  {} 2>&1) || error_exit "Failed to create ovsbr0 interface: $IFACE0_RESULT"

log "${GREEN}[SUCCESS]${NC} ovsbr0 interface created"
echo "$IFACE0_RESULT"
echo ""

# Create ovsbr1 bridge (isolated, no ports)
log "${YELLOW}[INFO]${NC} Creating ovsbr1 bridge via gdbus..."
BRIDGE1_RESULT=$(gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager/Settings \
  --method org.freedesktop.NetworkManager.Settings.AddConnection2 \
  "{'connection': {'id': <'ovsbr1'>, 'type': <'ovs-bridge'>, 'interface-name': <'ovsbr1'>, 'autoconnect': <true>}, 'ovs-bridge': {'stp-enable': <false>, 'mcast-snooping-enable': <true>}, 'ipv4': {'method': <'manual'>, 'address-data': <[{'address': '80.209.242.196', 'prefix': <uint32 25>}]>, 'gateway': <'80.209.242.129'>, 'dns': <[uint32 $DNS1, uint32 $DNS2]>}, 'ipv6': {'method': <'disabled'>}}" \
  1 \
  {} 2>&1) || error_exit "Failed to create ovsbr1: $BRIDGE1_RESULT"

log "${GREEN}[SUCCESS]${NC} ovsbr1 created"
echo "$BRIDGE1_RESULT"
echo ""

log "${GREEN}[SUCCESS]${NC} All bridges created via gdbus!"
echo ""

# Wait a moment for NetworkManager to settle
sleep 2

log ""
log "${BLUE}========================================${NC}"
log "${BLUE} PHASE 2: Plugin state management${NC}"
log "${BLUE}========================================${NC}"
log ""

# Check if nm-monitor/plugin is installed
if command -v ovs-port-agent &> /dev/null; then
    log "${GREEN}[INFO]${NC} nm-monitor plugin detected"
    
    # Query current state via plugin
    log "${YELLOW}[INFO]${NC} Querying current network state via plugin..."
    ovs-port-agent /etc/ovs-port-agent/config.toml query-state || true
    echo ""
    
    # Show diff against desired config
    if [[ -f "$CONFIG_FILE" ]]; then
        TEMP_CONFIG="/tmp/ovs-bridges-config-$$.yaml"
        sed "s/ens1/${PRIMARY_IFACE}/g" "$CONFIG_FILE" > "$TEMP_CONFIG"
        
        log "${YELLOW}[INFO]${NC} Checking state diff..."
        ovs-port-agent /etc/ovs-port-agent/config.toml show-diff "$TEMP_CONFIG" || true
        rm -f "$TEMP_CONFIG"
        echo ""
    fi
else
    log "${YELLOW}[INFO]${NC} nm-monitor plugin not installed (optional)"
fi

# Show final state
echo ""
log "${BLUE}[INFO]${NC} Final network state:"
echo ""

log "${YELLOW}=== IP Addresses ===${NC}"
ip addr show
echo ""

log "${YELLOW}=== Routing Table ===${NC}"
ip route show
echo ""

if command -v ovs-vsctl &> /dev/null; then
    log "${YELLOW}=== OVS Configuration ===${NC}"
    ovs-vsctl show
    echo ""
fi

log "${GREEN}========================================${NC}"
log "${GREEN} OVS Bridge Setup Complete!${NC}"
log "${GREEN}========================================${NC}"
log ""
log "Bridges created via gdbus:"
log "  • ovsbr0: 80.209.240.244/24 (gateway: 80.209.240.129)"
log "  • ovsbr1: 80.209.242.196/25 (gateway: 80.209.242.129)"
log ""
log "Your uplink ($PRIMARY_IFACE) is now attached to ovsbr0"
log ""
if command -v ovs-port-agent &> /dev/null; then
    log "State is now managed by plugin system"
    log "  - Query: sudo ovs-port-agent /etc/ovs-port-agent/config.toml query-state"
    log "  - Diff: sudo ovs-port-agent /etc/ovs-port-agent/config.toml show-diff <config>"
fi
log ""
