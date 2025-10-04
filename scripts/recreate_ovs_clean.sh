#!/usr/bin/env bash
# Clean recreation of OVS bridges with NetworkManager

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

BRIDGE="${1:-ovsbr0}"
IP="${2:-}"
GATEWAY="${3:-}"

echo "=== Clean OVS Bridge Recreation ==="
echo "Bridge: $BRIDGE"
[[ -n "$IP" ]] && echo "IP: $IP"
[[ -n "$GATEWAY" ]] && echo "Gateway: $GATEWAY"
echo

# 1. Complete cleanup
log_info "Step 1: Complete cleanup of $BRIDGE"

# Remove all NetworkManager connections for this bridge
log_info "Removing NetworkManager connections..."
for conn in $(nmcli -t -f NAME connection show | grep -E "(${BRIDGE}|ovs-.*${BRIDGE}|port.*${BRIDGE}|iface.*${BRIDGE})"); do
    log_info "  Deleting connection: $conn"
    nmcli connection delete "$conn" 2>/dev/null || true
done

# Remove from OVS
if ovs-vsctl br-exists "$BRIDGE" 2>/dev/null; then
    log_info "Removing bridge from Open vSwitch..."
    ovs-vsctl del-br "$BRIDGE" || true
fi

# Wait for cleanup
sleep 2

# 2. Create fresh following Example 20
log_info "Step 2: Creating bridge following NetworkManager documentation"

echo "Creating OVS bridge..."
nmcli conn add type ovs-bridge conn.interface "$BRIDGE" || {
    log_error "Failed to create bridge"
    exit 1
}

echo "Creating OVS port..."
nmcli conn add type ovs-port conn.interface "port0" controller "$BRIDGE" || {
    log_error "Failed to create port"
    exit 1
}

echo "Creating OVS interface..."
if [[ -n "$IP" ]]; then
    # With IP
    CMD=(nmcli conn add type ovs-interface port-type ovs-port 
         conn.interface "iface0" controller "port0"
         ipv4.method manual ipv4.address "$IP")
    
    [[ -n "$GATEWAY" ]] && CMD+=(ipv4.gateway "$GATEWAY")
    
    "${CMD[@]}" || {
        log_error "Failed to create interface"
        exit 1
    }
else
    # Without IP
    nmcli conn add type ovs-interface port-type ovs-port \
        conn.interface "iface0" controller "port0" \
        ipv4.method disabled || {
        log_error "Failed to create interface"
        exit 1
    }
fi

# 3. Add to OVS
log_info "Step 3: Adding bridge to Open vSwitch"
ovs-vsctl --may-exist add-br "$BRIDGE" || {
    log_error "Failed to add bridge to OVS"
    exit 1
}

# 4. Show current state
echo
log_info "Current state:"
echo "Devices:"
nmcli device status | grep -E "(DEVICE|${BRIDGE}|port0|iface0)" || true
echo
echo "Connections:"
nmcli connection show | grep -E "(NAME|ovs-.*${BRIDGE}|ovs-.*port0|ovs-.*iface0)" || true
echo
echo "OVS:"
ovs-vsctl show | grep -A 5 "Bridge.*${BRIDGE}" || true

echo
log_info "To activate: nmcli connection up ovs-bridge-${BRIDGE}"