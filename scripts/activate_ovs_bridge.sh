#!/usr/bin/env bash
# Activate OVS bridge with proper ordering
# This script handles the specific activation sequence needed for OVS bridges

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

log_info "Activating OVS bridge: $BRIDGE"

# 1. Ensure OVS service is running
if ! systemctl is-active --quiet openvswitch-switch 2>/dev/null && ! systemctl is-active --quiet openvswitch 2>/dev/null; then
    log_error "Open vSwitch service is not running"
    log_info "Starting Open vSwitch..."
    systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null || {
        log_error "Failed to start Open vSwitch"
        exit 1
    }
    sleep 2
fi

# 2. Ensure bridge exists in OVS
if ! ovs-vsctl br-exists "$BRIDGE" 2>/dev/null; then
    log_info "Adding bridge $BRIDGE to Open vSwitch"
    ovs-vsctl add-br "$BRIDGE"
fi

# 3. Check connections exist
PORT_INT="${BRIDGE}-port-int"
IF_NAME="${BRIDGE}-if"

for conn in "$BRIDGE" "$PORT_INT" "$IF_NAME"; do
    if ! nmcli connection show "$conn" >/dev/null 2>&1; then
        log_error "Connection $conn not found!"
        exit 1
    fi
done

# 4. Deactivate all first to ensure clean state
log_info "Ensuring clean state..."
for conn in "$IF_NAME" "$PORT_INT" "$BRIDGE"; do
    nmcli connection down "$conn" 2>/dev/null || true
done

sleep 1

# 5. Activate in proper order
log_info "Activating bridge..."
nmcli connection up "$BRIDGE" || {
    log_error "Failed to activate bridge"
    exit 1
}

# Wait for bridge to be ready
sleep 2

# 6. Check if slaves auto-activated
PORT_STATE=$(nmcli -t -f GENERAL.STATE connection show "$PORT_INT" 2>/dev/null | cut -d: -f2 || echo "unknown")
IF_STATE=$(nmcli -t -f GENERAL.STATE connection show "$IF_NAME" 2>/dev/null | cut -d: -f2 || echo "unknown")

log_info "Port state: $PORT_STATE"
log_info "Interface state: $IF_STATE"

# 7. Manually activate if needed
if [[ "$PORT_STATE" != "activated" ]]; then
    log_info "Manually activating port..."
    nmcli connection up "$PORT_INT" || log_warn "Port activation failed"
fi

if [[ "$IF_STATE" != "activated" ]]; then
    log_info "Manually activating interface..."
    nmcli connection up "$IF_NAME" || log_warn "Interface activation failed"
fi

# 8. Final check
sleep 2
log_info "Final status:"
nmcli -t -f NAME,TYPE,STATE,DEVICE connection show | grep -E "(${BRIDGE}|ovs-)" | column -t -s:

# Check IP
IP_ADDR=$(ip -4 addr show "$BRIDGE" 2>/dev/null | grep inet | awk '{print $2}' || echo "none")
log_info "Bridge IP: $IP_ADDR"

if [[ "$IP_ADDR" == "none" ]]; then
    log_warn "No IP address on bridge"
    log_info "Checking interface configuration:"
    nmcli -f ipv4.addresses,ipv4.method connection show "$IF_NAME"
fi

log_info "Activation complete"