#!/usr/bin/env bash
# Fix unmanaged OVS bridges by creating proper NetworkManager connections

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

echo "=== Fixing Unmanaged OVS Bridges ==="
echo

# 1. Check current state
log_info "Current device state:"
nmcli device status | grep -E "(DEVICE|ovs)" || true
echo

log_info "Current connections:"
nmcli connection show | grep -E "(NAME|ovs)" || true
echo

# 2. Check what's in OVS
log_info "Open vSwitch state:"
if command -v ovs-vsctl >/dev/null 2>&1; then
    ovs-vsctl show || true
else
    log_error "ovs-vsctl not found"
fi
echo

# 3. Find unmanaged OVS bridges
log_info "Finding unmanaged OVS bridges..."
UNMANAGED_BRIDGES=$(nmcli -t -f DEVICE,TYPE,STATE device status | grep "ovs-bridge:unmanaged" | cut -d: -f1 | sort -u)

if [[ -z "$UNMANAGED_BRIDGES" ]]; then
    log_info "No unmanaged OVS bridges found"
    exit 0
fi

echo "Found unmanaged bridges:"
echo "$UNMANAGED_BRIDGES"
echo

# 4. For each unmanaged bridge, create NetworkManager connections
for bridge in $UNMANAGED_BRIDGES; do
    log_warn "Bridge $bridge is unmanaged by NetworkManager"
    
    # Check if bridge exists in OVS
    if ! ovs-vsctl br-exists "$bridge" 2>/dev/null; then
        log_error "Bridge $bridge not found in Open vSwitch"
        continue
    fi
    
    echo
    read -p "Create NetworkManager connections for $bridge? (y/n) " -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Skipping $bridge"
        continue
    fi
    
    log_info "Creating NetworkManager connections for $bridge"
    
    # Following Example 20 from NetworkManager documentation
    
    # 1. Create bridge connection
    log_info "Creating bridge connection..."
    nmcli conn add type ovs-bridge conn.interface "$bridge" || {
        log_error "Failed to create bridge connection"
        continue
    }
    
    # 2. Create internal port
    log_info "Creating internal port..."
    nmcli conn add type ovs-port conn.interface "port-int-$bridge" controller "$bridge" || {
        log_error "Failed to create internal port"
        continue
    }
    
    # 3. Create interface (this is what gets the IP)
    log_info "Creating interface..."
    
    # Check if the bridge currently has an IP
    CURRENT_IP=$(ip -4 addr show "$bridge" 2>/dev/null | grep "inet " | awk '{print $2}' | head -1)
    
    if [[ -n "$CURRENT_IP" ]]; then
        log_info "Found existing IP on $bridge: $CURRENT_IP"
        
        # Get gateway if any
        CURRENT_GW=$(ip route | grep "default.*dev $bridge" | awk '{print $3}' | head -1)
        
        nmcli conn add type ovs-interface conn.interface "$bridge" \
            controller "port-int-$bridge" \
            ipv4.method manual ipv4.address "$CURRENT_IP" \
            ${CURRENT_GW:+ipv4.gateway "$CURRENT_GW"} || {
            log_error "Failed to create interface with IP"
            continue
        }
    else
        log_info "No IP found on $bridge, creating without IP"
        nmcli conn add type ovs-interface conn.interface "$bridge" \
            controller "port-int-$bridge" \
            ipv4.method disabled || {
            log_error "Failed to create interface"
            continue
        }
    fi
    
    log_info "NetworkManager connections created for $bridge"
done

echo
log_info "Current state after fixes:"
nmcli device status | grep -E "(DEVICE|ovs)" || true
echo
nmcli connection show | grep -E "(NAME|ovs)" || true

echo
log_info "To activate a bridge: nmcli connection up ovs-bridge-<bridge-name>"