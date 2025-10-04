#!/usr/bin/env bash
# Setup OVS bridge following NetworkManager documentation strictly
# Handles IP migration from uplink to ovs-interface properly

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Parameters
BRIDGE="${1:-ovsbr0}"
UPLINK="${2:-}"
MANUAL_IP="${3:-}"
MANUAL_GW="${4:-}"

if [[ -z "$UPLINK" && -z "$MANUAL_IP" ]]; then
    echo "Usage: $0 BRIDGE UPLINK [MANUAL_IP] [MANUAL_GW]"
    echo "   or: $0 BRIDGE \"\" MANUAL_IP MANUAL_GW"
    echo
    echo "Examples:"
    echo "  $0 ovsbr0 eth0                    # Migrate IP from eth0"
    echo "  $0 ovsbr0 eth0 192.168.1.10/24   # Use manual IP, eth0 as uplink"
    echo "  $0 ovsbr0 \"\" 192.168.1.10/24 192.168.1.1  # No uplink, manual IP"
    exit 1
fi

log_info "Setting up OVS bridge $BRIDGE"

# Ensure OVS is running
systemctl is-active --quiet openvswitch-switch 2>/dev/null || systemctl is-active --quiet openvswitch 2>/dev/null || {
    log_info "Starting Open vSwitch"
    systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null
    sleep 2
}

# Add bridge to OVS
ovs-vsctl --may-exist add-br "$BRIDGE"

# Variables for IP configuration
IP_METHOD="disabled"
IP_ADDRESSES=""
IP_GATEWAY=""
IP_DNS=""

# Introspect IP configuration from uplink if present
if [[ -n "$UPLINK" ]]; then
    log_info "Checking for active connection on $UPLINK"
    
    # Find active connection on uplink
    ACTIVE_CONN=$(nmcli -t -f NAME,DEVICE,STATE connection show --active | \
        grep ":${UPLINK}:activated$" | cut -d: -f1 | head -1)
    
    if [[ -n "$ACTIVE_CONN" ]]; then
        log_info "Found active connection: $ACTIVE_CONN"
        
        # Introspect IP configuration
        IP_METHOD=$(nmcli -t -f ipv4.method connection show "$ACTIVE_CONN" | cut -d: -f2)
        
        if [[ "$IP_METHOD" == "manual" || "$IP_METHOD" == "auto" ]]; then
            log_info "Introspecting IP configuration from $ACTIVE_CONN"
            
            IP_ADDRESSES=$(nmcli -t -f ipv4.addresses connection show "$ACTIVE_CONN" | cut -d: -f2)
            IP_GATEWAY=$(nmcli -t -f ipv4.gateway connection show "$ACTIVE_CONN" | cut -d: -f2)
            IP_DNS=$(nmcli -t -f ipv4.dns connection show "$ACTIVE_CONN" | cut -d: -f2)
            
            log_info "  Method: $IP_METHOD"
            log_info "  Address: $IP_ADDRESSES"
            [[ -n "$IP_GATEWAY" ]] && log_info "  Gateway: $IP_GATEWAY"
            [[ -n "$IP_DNS" ]] && log_info "  DNS: $IP_DNS"
        fi
    fi
fi

# Override with manual configuration if provided
if [[ -n "$MANUAL_IP" ]]; then
    log_info "Using manual IP configuration"
    IP_METHOD="manual"
    IP_ADDRESSES="$MANUAL_IP"
    [[ -n "$MANUAL_GW" ]] && IP_GATEWAY="$MANUAL_GW"
fi

# Clean up existing bridge connections
log_info "Cleaning up existing connections"
for conn in $(nmcli -t -f NAME connection show | grep "^${BRIDGE}"); do
    nmcli connection delete "$conn" 2>/dev/null || true
done

log_info "Creating OVS bridge topology"

# 1. Create bridge connection
nmcli connection add \
    type ovs-bridge \
    con-name "$BRIDGE" \
    ifname "$BRIDGE" \
    ovs-bridge.stp no \
    ovs-bridge.rstp no \
    connection.autoconnect yes \
    connection.autoconnect-priority 100

# 2. Create internal port
nmcli connection add \
    type ovs-port \
    con-name "${BRIDGE}-port-int" \
    ifname "$BRIDGE" \
    connection.master "$BRIDGE" \
    connection.slave-type ovs-bridge \
    connection.autoconnect yes \
    connection.autoconnect-priority 95

# 3. Create ovs-interface with IP configuration
log_info "Creating ovs-interface with IP configuration"
CMD=(nmcli connection add
    type ovs-interface
    con-name "${BRIDGE}-if"
    ifname "$BRIDGE"
    connection.master "${BRIDGE}-port-int"
    connection.slave-type ovs-port
    connection.autoconnect yes
    connection.autoconnect-priority 95
    ovs-interface.type internal)

# Add IP configuration to ovs-interface
if [[ "$IP_METHOD" == "manual" && -n "$IP_ADDRESSES" ]]; then
    CMD+=(
        ipv4.method manual
        ipv4.addresses "$IP_ADDRESSES"
    )
    [[ -n "$IP_GATEWAY" ]] && CMD+=(ipv4.gateway "$IP_GATEWAY")
    [[ -n "$IP_DNS" ]] && CMD+=(ipv4.dns "$IP_DNS")
else
    CMD+=(ipv4.method disabled)
fi

CMD+=(ipv6.method disabled)

"${CMD[@]}"

# 4. Handle uplink if provided
if [[ -n "$UPLINK" ]]; then
    log_info "Creating uplink port for $UPLINK"
    
    # Create port
    nmcli connection add \
        type ovs-port \
        con-name "${BRIDGE}-port-${UPLINK}" \
        ifname "$UPLINK" \
        connection.master "$BRIDGE" \
        connection.slave-type ovs-bridge \
        connection.autoconnect yes \
        connection.autoconnect-priority 90
    
    # Handle the physical interface connection
    if [[ -n "$ACTIVE_CONN" ]]; then
        log_info "Migrating $ACTIVE_CONN to OVS slave (removing IP configuration)"
        
        # Remove IP configuration from physical interface
        nmcli connection modify "$ACTIVE_CONN" \
            ipv4.method disabled \
            ipv4.addresses "" \
            ipv4.gateway "" \
            ipv4.dns "" \
            ipv6.method disabled \
            connection.master "${BRIDGE}-port-${UPLINK}" \
            connection.slave-type ovs-port \
            connection.autoconnect yes \
            connection.autoconnect-priority 85
    else
        # Create new ethernet slave
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

# Activate the bridge
log_info "Activating bridge (NetworkManager handles slaves atomically)"
nmcli connection up "$BRIDGE" || {
    log_error "Failed to activate bridge"
    exit 1
}

# Verify
sleep 3
log_info "Verification:"
echo
log_info "Connection status:"
nmcli -t -f NAME,TYPE,STATE,DEVICE connection show | grep -E "(${BRIDGE}|ovs-)" | column -t -s:

echo
log_info "IP configuration on bridge:"
ip addr show "$BRIDGE" | grep inet || echo "  No IP address"

echo
log_info "Bridge details:"
ovs-vsctl show | grep -A 10 "Bridge.*$BRIDGE"

log_info "Complete!"