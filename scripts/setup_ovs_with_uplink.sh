#!/usr/bin/env bash
# Setup OVS bridge with uplink in a way that maintains connectivity
# This is critical for remote systems where the uplink is the primary network interface

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
log_step() { echo -e "${BLUE}[STEP]${NC} $*"; }

# Parse arguments
BRIDGE="${1:-ovsbr0}"
UPLINK="${2:-}"
IP="${3:-}"
GATEWAY="${4:-}"

if [[ -z "$UPLINK" ]]; then
    echo "Usage: $0 BRIDGE UPLINK [IP/MASK] [GATEWAY]"
    echo "Example: $0 ovsbr0 ens1 192.168.1.10/24 192.168.1.1"
    echo
    echo "If IP/GATEWAY are not provided, they will be detected from the uplink"
    exit 1
fi

log_info "Setting up OVS bridge $BRIDGE with uplink $UPLINK"

# Check if uplink exists and is active
if ! ip link show "$UPLINK" >/dev/null 2>&1; then
    log_error "Uplink interface $UPLINK does not exist"
    exit 1
fi

# Detect current IP configuration if not provided
if [[ -z "$IP" ]]; then
    log_info "Detecting IP configuration from $UPLINK..."
    
    # Get IPv4 address
    IP=$(ip -4 addr show "$UPLINK" | grep -o 'inet [0-9./]*' | awk '{print $2}' | head -1)
    if [[ -z "$IP" ]]; then
        log_error "No IPv4 address found on $UPLINK"
        exit 1
    fi
    log_info "Detected IP: $IP"
    
    # Get gateway
    if [[ -z "$GATEWAY" ]]; then
        GATEWAY=$(ip route | grep "^default.*dev $UPLINK" | awk '{print $3}' | head -1)
        if [[ -n "$GATEWAY" ]]; then
            log_info "Detected gateway: $GATEWAY"
        else
            log_warn "No gateway detected"
        fi
    fi
fi

# Get active NetworkManager connection on uplink
ACTIVE_CONN=$(nmcli -t -f NAME,DEVICE,STATE connection show --active | grep ":${UPLINK}:activated$" | cut -d: -f1 | head -1)
if [[ -n "$ACTIVE_CONN" ]]; then
    log_info "Found active connection '$ACTIVE_CONN' on $UPLINK"
else
    log_warn "No active NetworkManager connection found on $UPLINK"
fi

log_step "Creating OVS bridge configuration"

# Ensure OVS service is running
if ! systemctl is-active --quiet openvswitch-switch 2>/dev/null && ! systemctl is-active --quiet openvswitch 2>/dev/null; then
    log_info "Starting Open vSwitch..."
    systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null
    sleep 2
fi

# Clean up any existing bridge configuration
log_info "Cleaning up any existing configuration..."
for conn in $(nmcli -t -f NAME connection show | grep "^${BRIDGE}"); do
    nmcli connection delete "$conn" 2>/dev/null || true
done
ovs-vsctl del-br "$BRIDGE" 2>/dev/null || true

# Create all connections FIRST, before any activation
log_step "Creating NetworkManager connections"

# 1. Create bridge
log_info "Creating bridge connection..."
nmcli connection add \
    type ovs-bridge \
    con-name "$BRIDGE" \
    ifname "$BRIDGE" \
    ovs-bridge.stp no \
    ovs-bridge.rstp no \
    connection.autoconnect yes \
    connection.autoconnect-priority 100

# 2. Create internal port
log_info "Creating internal port..."
nmcli connection add \
    type ovs-port \
    con-name "${BRIDGE}-port-int" \
    ifname "$BRIDGE" \
    connection.master "$BRIDGE" \
    connection.slave-type ovs-bridge \
    connection.autoconnect yes \
    connection.autoconnect-priority 95

# 3. Create interface with IP
log_info "Creating interface with IP configuration..."
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

if [[ -n "$GATEWAY" ]]; then
    nmcli connection modify "${BRIDGE}-if" ipv4.gateway "$GATEWAY"
fi

# 4. Create uplink port
log_info "Creating uplink port..."
nmcli connection add \
    type ovs-port \
    con-name "${BRIDGE}-port-${UPLINK}" \
    ifname "$UPLINK" \
    connection.master "$BRIDGE" \
    connection.slave-type ovs-bridge \
    connection.autoconnect yes \
    connection.autoconnect-priority 90

# 5. Handle uplink ethernet connection
if [[ -n "$ACTIVE_CONN" ]]; then
    log_info "Converting existing connection '$ACTIVE_CONN' to OVS slave..."
    
    # First, we need to clear any IP configuration from the physical interface
    nmcli connection modify "$ACTIVE_CONN" \
        ipv4.method disabled \
        ipv4.addresses "" \
        ipv4.gateway "" \
        ipv6.method disabled
    
    # Then make it a slave
    nmcli connection modify "$ACTIVE_CONN" \
        connection.master "${BRIDGE}-port-${UPLINK}" \
        connection.slave-type ovs-port \
        connection.autoconnect yes \
        connection.autoconnect-priority 85
else
    log_info "Creating ethernet slave connection..."
    nmcli connection add \
        type ethernet \
        con-name "${BRIDGE}-eth-${UPLINK}" \
        ifname "$UPLINK" \
        connection.master "${BRIDGE}-port-${UPLINK}" \
        connection.slave-type ovs-port \
        connection.autoconnect yes \
        connection.autoconnect-priority 85
fi

# Add bridge to OVS
log_step "Adding bridge to Open vSwitch"
ovs-vsctl add-br "$BRIDGE"

# Now do the critical activation
log_step "Performing atomic network transition"
log_warn "Network connectivity may be briefly interrupted"

# If we have an active connection, we need to do this carefully
if [[ -n "$ACTIVE_CONN" ]]; then
    log_info "Performing hot transition from $ACTIVE_CONN to bridge..."
    
    # The trick is to activate the bridge which will automatically
    # transition the uplink due to the master/slave relationship
    nmcli connection up "$BRIDGE" || {
        log_error "Failed to activate bridge!"
        log_info "Attempting to restore original connection..."
        nmcli connection up "$ACTIVE_CONN" 2>/dev/null || true
        exit 1
    }
else
    # No active connection, just bring up the bridge
    nmcli connection up "$BRIDGE" || {
        log_error "Failed to activate bridge!"
        exit 1
    }
fi

# Verify
sleep 3
log_step "Verifying configuration"

# Check bridge state
BRIDGE_STATE=$(nmcli -t -f STATE connection show "$BRIDGE" | cut -d: -f2)
if [[ "$BRIDGE_STATE" == "activated" ]]; then
    log_info "✅ Bridge is active"
else
    log_error "❌ Bridge is not active (state: $BRIDGE_STATE)"
fi

# Check IP
ACTUAL_IP=$(ip -4 addr show "$BRIDGE" | grep -o 'inet [0-9./]*' | awk '{print $2}')
if [[ "$ACTUAL_IP" == "$IP" ]]; then
    log_info "✅ IP address configured correctly: $ACTUAL_IP"
else
    log_error "❌ IP address mismatch: expected $IP, got $ACTUAL_IP"
fi

# Check connectivity
if [[ -n "$GATEWAY" ]]; then
    if ping -c 1 -W 2 "$GATEWAY" >/dev/null 2>&1; then
        log_info "✅ Gateway is reachable"
    else
        log_warn "⚠️  Gateway is not reachable"
    fi
fi

# Show final status
echo
log_info "Final configuration:"
nmcli device status | grep -E "(DEVICE|$BRIDGE|$UPLINK)"
echo
ip addr show "$BRIDGE" | grep inet || true

log_info "Setup complete!"

# Important note for remote systems
if [[ -n "$SSH_CONNECTION" ]]; then
    echo
    log_warn "You are connected via SSH. If you lose connectivity:"
    log_warn "1. Wait 30 seconds for NetworkManager to stabilize"
    log_warn "2. The system should be accessible at the same IP: $IP"
    log_warn "3. If not, you may need console access to troubleshoot"
fi