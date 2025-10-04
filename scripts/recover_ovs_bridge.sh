#!/usr/bin/env bash
# Recover OVS bridge from failed activation state

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

echo "=== OVS Bridge Recovery for $BRIDGE ==="
echo

# 1. Stop all related connections
log_info "Stopping all related connections..."
nmcli -t -f NAME,TYPE connection show | grep -E "(^${BRIDGE}[:-]|^${BRIDGE}$)" | cut -d: -f1 | while IFS= read -r conn; do
    log_info "  Stopping: $conn"
    nmcli connection down "$conn" 2>/dev/null || true
done

# 2. Ensure OVS has the bridge
log_info "Checking OVS database..."
if ! ovs-vsctl br-exists "$BRIDGE" 2>/dev/null; then
    log_warn "Bridge $BRIDGE not in OVS, adding..."
    ovs-vsctl add-br "$BRIDGE"
else
    log_info "Bridge exists in OVS"
fi

# 3. Clear any failed state
log_info "Clearing NetworkManager state..."
# This forces NM to re-read the connection
nmcli connection reload

# 4. Check connection hierarchy
log_info "Verifying connection hierarchy..."

# Check bridge
if ! nmcli connection show "$BRIDGE" >/dev/null 2>&1; then
    log_error "Bridge connection $BRIDGE not found!"
    exit 1
fi

# Check internal port
PORT_INT="${BRIDGE}-port-int"
if ! nmcli connection show "$PORT_INT" >/dev/null 2>&1; then
    log_error "Internal port connection $PORT_INT not found!"
    exit 1
fi

# Check interface
IF_NAME="${BRIDGE}-if"
if ! nmcli connection show "$IF_NAME" >/dev/null 2>&1; then
    log_error "Interface connection $IF_NAME not found!"
    exit 1
fi

# 5. Verify master/slave relationships
log_info "Checking master/slave relationships..."

# Port should have bridge as master
PORT_MASTER=$(nmcli -t -f connection.master connection show "$PORT_INT" | cut -d: -f2)
if [[ "$PORT_MASTER" != "$BRIDGE" ]]; then
    log_warn "Fixing port master relationship"
    nmcli connection modify "$PORT_INT" connection.master "$BRIDGE"
fi

# Interface should have port as master
IF_MASTER=$(nmcli -t -f connection.master connection show "$IF_NAME" | cut -d: -f2)
if [[ "$IF_MASTER" != "$PORT_INT" ]]; then
    log_warn "Fixing interface master relationship"
    nmcli connection modify "$IF_NAME" connection.master "$PORT_INT"
fi

# 6. Try activation in steps
log_info "Attempting step-by-step activation..."

# First, just the bridge
log_info "Step 1: Activating bridge alone..."
nmcli connection up "$BRIDGE" 2>&1 | tee /tmp/bridge-up.log || {
    log_warn "Bridge activation failed, continuing..."
}

sleep 2

# Then the internal port
log_info "Step 2: Activating internal port..."
nmcli connection up "$PORT_INT" 2>&1 | tee /tmp/port-up.log || {
    log_warn "Port activation failed, continuing..."
}

sleep 2

# Finally the interface
log_info "Step 3: Activating interface..."
nmcli connection up "$IF_NAME" 2>&1 | tee /tmp/if-up.log || {
    log_warn "Interface activation failed"
    
    # Check if it's an IP conflict
    if grep -q "no available address" /tmp/if-up.log; then
        log_error "IP address conflict detected!"
        log_info "Current IP configuration:"
        nmcli -t -f ipv4.addresses,ipv4.gateway connection show "$IF_NAME"
    fi
}

# 7. Final status check
echo
log_info "Final status:"
nmcli -t -f NAME,TYPE,STATE,DEVICE connection show | grep -E "(${BRIDGE}|ovs-)" | column -t -s:

# 8. Show any errors
echo
log_info "Recent errors from NetworkManager:"
journalctl -u NetworkManager -n 20 --no-pager | grep -E "error|fail" -i | tail -5 || echo "  No recent errors"

echo
log_info "Recovery attempt complete"

# Provide next steps
echo
echo "=== Next Steps ==="
echo "1. Check the status above"
echo "2. If still failing, check: journalctl -u NetworkManager -f"
echo "3. Try manual activation: nmcli connection up $BRIDGE"
echo "4. Run diagnostics: ./scripts/diagnose_ovs_bridge.sh --bridge $BRIDGE"