#!/usr/bin/env bash
# Quick setup script for OVS bridge with NetworkManager
# Handles the most common issues automatically

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Default values from command line
BRIDGE="${1:-ovsbr0}"
IP="${2:-}"
GATEWAY="${3:-}"
UPLINK="${4:-}"

if [[ -z "$IP" ]]; then
    echo "Usage: $0 BRIDGE IP/MASK [GATEWAY] [UPLINK]"
    echo "Example: $0 ovsbr0 192.168.1.10/24 192.168.1.1 eth0"
    exit 1
fi

log_info "Quick OVS bridge setup: $BRIDGE with IP $IP"

# 1. Ensure services are running
log_info "Checking services..."
systemctl is-active --quiet NetworkManager || {
    log_warn "Starting NetworkManager..."
    systemctl start NetworkManager
    sleep 2
}

systemctl is-active --quiet openvswitch-switch 2>/dev/null || systemctl is-active --quiet openvswitch 2>/dev/null || {
    log_warn "Starting Open vSwitch..."
    systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null
    sleep 2
}

# 2. Clean up any existing configuration
log_info "Cleaning up existing configuration..."
# Delete NM connections
nmcli -t -f NAME,TYPE connection show | grep -E "(^${BRIDGE}[:-]|^${BRIDGE}$)" | cut -d: -f1 | while IFS= read -r conn; do
    log_info "  Deleting connection: $conn"
    nmcli connection delete "$conn" 2>/dev/null || true
done

# Remove from OVS if exists
if ovs-vsctl br-exists "$BRIDGE" 2>/dev/null; then
    log_info "  Removing bridge from OVS"
    ovs-vsctl del-br "$BRIDGE" || true
fi

# 3. Create bridge in OVS first
log_info "Creating bridge in Open vSwitch..."
ovs-vsctl add-br "$BRIDGE"

# 4. Create NetworkManager connections
log_info "Creating NetworkManager connections..."

# Bridge
nmcli connection add \
    type ovs-bridge \
    con-name "$BRIDGE" \
    ifname "$BRIDGE" \
    ovs-bridge.stp no \
    ovs-bridge.rstp no \
    connection.autoconnect yes \
    connection.autoconnect-priority 100

# Internal port
nmcli connection add \
    type ovs-port \
    con-name "${BRIDGE}-port-int" \
    ifname "$BRIDGE" \
    connection.master "$BRIDGE" \
    connection.slave-type ovs-bridge \
    connection.autoconnect yes \
    connection.autoconnect-priority 95

# Interface with IP
nmcli connection add \
    type ovs-interface \
    con-name "${BRIDGE}-if" \
    ifname "$BRIDGE" \
    connection.master "${BRIDGE}-port-int" \
    connection.slave-type ovs-port \
    connection.autoconnect yes \
    connection.autoconnect-priority 95 \
    ovs-interface.type internal \
    ipv4.method manual \
    ipv4.addresses "$IP" \
    ipv6.method disabled

# Add gateway if provided
if [[ -n "$GATEWAY" ]]; then
    nmcli connection modify "${BRIDGE}-if" ipv4.gateway "$GATEWAY"
fi

# 5. Add uplink if provided
if [[ -n "$UPLINK" ]]; then
    log_info "Adding uplink $UPLINK..."
    
    # Check if uplink has an active connection
    ACTIVE_CONN=$(nmcli -t -f NAME,DEVICE,ACTIVE connection show | grep ":${UPLINK}:yes$" | cut -d: -f1 | head -1)
    
    # Create port for uplink
    nmcli connection add \
        type ovs-port \
        con-name "${BRIDGE}-port-${UPLINK}" \
        ifname "$UPLINK" \
        connection.master "$BRIDGE" \
        connection.slave-type ovs-bridge \
        connection.autoconnect yes \
        connection.autoconnect-priority 90
    
    if [[ -n "$ACTIVE_CONN" ]]; then
        log_info "Migrating active connection '$ACTIVE_CONN' to OVS slave"
        nmcli connection modify "$ACTIVE_CONN" \
            connection.master "${BRIDGE}-port-${UPLINK}" \
            connection.slave-type ovs-port \
            connection.autoconnect yes \
            connection.autoconnect-priority 85
    else
        # Create ethernet slave
        nmcli connection add \
            type ethernet \
            con-name "${BRIDGE}-eth-${UPLINK}" \
            ifname "$UPLINK" \
            connection.master "${BRIDGE}-port-${UPLINK}" \
            connection.slave-type ovs-port \
            connection.autoconnect yes \
            connection.autoconnect-priority 85
    fi
fi

# 6. Activate the bridge
log_info "Activating bridge..."
nmcli connection up "$BRIDGE" || {
    log_error "Failed to activate bridge"
    log_info "Checking logs..."
    journalctl -u NetworkManager -n 10 --no-pager | grep -i ovs || true
    exit 1
}

# 7. Verify
sleep 2
log_info "Verifying configuration..."
if nmcli -t -f GENERAL.STATE connection show "$BRIDGE" | grep -q ":activated$"; then
    log_info "✅ Bridge activated successfully"
else
    log_warn "Bridge may not be fully active"
fi

# Show status
echo
log_info "Current status:"
nmcli device status | grep -E "(DEVICE|$BRIDGE)" || true
echo
ip addr show "$BRIDGE" 2>/dev/null || true

log_info "Setup complete!"