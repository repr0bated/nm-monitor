#!/usr/bin/env bash
# Create OVS bridge with atomic setup - all connections created before activation
# This ensures NetworkManager handles everything properly

set -euo pipefail

# Default values
BRIDGE="${1:-ovsbr0}"
IP="${2:-}"
GATEWAY="${3:-}"
UPLINK="${4:-}"

if [[ -z "$IP" ]]; then
    echo "Usage: $0 BRIDGE IP/MASK [GATEWAY] [UPLINK]"
    echo "Example: $0 ovsbr0 192.168.1.10/24 192.168.1.1 eth0"
    exit 1
fi

echo "[*] Creating atomic OVS bridge configuration for $BRIDGE"

# Ensure OVS is running
systemctl is-active --quiet openvswitch-switch 2>/dev/null || systemctl is-active --quiet openvswitch 2>/dev/null || {
    echo "[*] Starting Open vSwitch"
    systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null
    sleep 2
}

# Add bridge to OVS first
ovs-vsctl --may-exist add-br "$BRIDGE"

# Clean any existing connections
for conn in $(nmcli -t -f NAME connection show | grep "^${BRIDGE}"); do
    nmcli connection delete "$conn" 2>/dev/null || true
done

echo "[*] Creating NetworkManager connections..."

# Create base connections with autoconnect disabled
# We'll enable it only on the bridge to ensure atomic activation

# 1. Bridge (master) - autoconnect enabled
nmcli connection add \
    type ovs-bridge \
    con-name "$BRIDGE" \
    ifname "$BRIDGE" \
    ovs-bridge.stp no \
    ovs-bridge.rstp no \
    connection.autoconnect yes \
    connection.autoconnect-priority 100

# 2. Internal port - autoconnect disabled (slave)
nmcli connection add \
    type ovs-port \
    con-name "${BRIDGE}-port-int" \
    ifname "$BRIDGE" \
    connection.master "$BRIDGE" \
    connection.slave-type ovs-bridge \
    connection.autoconnect no

# 3. Internal interface with IP - autoconnect disabled (slave of slave)
CMD=(nmcli connection add \
    type ovs-interface \
    con-name "${BRIDGE}-if" \
    ifname "$BRIDGE" \
    connection.master "${BRIDGE}-port-int" \
    connection.slave-type ovs-port \
    connection.autoconnect no \
    ovs-interface.type internal \
    ipv4.method manual \
    ipv4.addresses "$IP" \
    ipv6.method disabled)

if [[ -n "$GATEWAY" ]]; then
    CMD+=(ipv4.gateway "$GATEWAY")
fi

"${CMD[@]}"

# 4. If uplink provided, add it BEFORE activation
if [[ -n "$UPLINK" ]]; then
    echo "[*] Adding uplink $UPLINK to bridge"
    
    # Create port for uplink
    nmcli connection add \
        type ovs-port \
        con-name "${BRIDGE}-port-${UPLINK}" \
        ifname "$UPLINK" \
        connection.master "$BRIDGE" \
        connection.slave-type ovs-bridge \
        connection.autoconnect no
    
    # Check for existing active connection
    ACTIVE_CONN=$(nmcli -t -f NAME,DEVICE,STATE connection show --active | \
        grep ":${UPLINK}:activated$" | cut -d: -f1 | head -1)
    
    if [[ -n "$ACTIVE_CONN" ]]; then
        echo "[*] Migrating active connection '$ACTIVE_CONN'"
        # Clear IP config from physical interface
        nmcli connection modify "$ACTIVE_CONN" \
            ipv4.method disabled \
            ipv4.addresses "" \
            ipv4.gateway "" \
            ipv6.method disabled \
            connection.master "${BRIDGE}-port-${UPLINK}" \
            connection.slave-type ovs-port \
            connection.autoconnect no
    else
        # Create ethernet slave
        nmcli connection add \
            type ethernet \
            con-name "${BRIDGE}-eth-${UPLINK}" \
            ifname "$UPLINK" \
            connection.master "${BRIDGE}-port-${UPLINK}" \
            connection.slave-type ovs-port \
            connection.autoconnect no
    fi
fi

echo "[*] Activating bridge (NetworkManager will handle all slaves atomically)"

# Now activate just the bridge - NM will bring up all slaves
if ! nmcli connection up "$BRIDGE"; then
    echo "[!] Failed to activate bridge"
    exit 1
fi

# Verify
sleep 2
echo "[*] Verification:"
nmcli -t -f NAME,TYPE,STATE,DEVICE connection show | grep -E "(${BRIDGE}|ovs-)" | column -t -s:
echo
ip addr show "$BRIDGE" 2>/dev/null | grep inet || echo "[!] No IP on bridge"

echo "[*] Complete!"