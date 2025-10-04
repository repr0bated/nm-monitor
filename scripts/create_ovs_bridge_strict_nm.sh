#!/usr/bin/env bash
# Create OVS bridge following NetworkManager documentation EXACTLY
# Based on Example 20 and 21 from NetworkManager.dev

set -euo pipefail

# Parameters
BRIDGE_NAME="${1:-bridge0}"
IP_ADDRESS="${2:-}"
UPLINK="${3:-}"

echo "Creating OVS bridge following NetworkManager documentation exactly"
echo

# Example 20: Creating a Bridge with a single internal Interface

echo "Step 1: Creating OVS bridge"
nmcli conn add type ovs-bridge conn.interface "$BRIDGE_NAME"

echo "Step 2: Creating OVS port for internal interface"
nmcli conn add type ovs-port conn.interface "port0" controller "$BRIDGE_NAME"

echo "Step 3: Creating OVS interface with IP"
if [[ -n "$IP_ADDRESS" ]]; then
    nmcli conn add type ovs-interface port-type ovs-port conn.interface "iface0" \
        controller "port0" ipv4.method manual ipv4.address "$IP_ADDRESS"
else
    nmcli conn add type ovs-interface port-type ovs-port conn.interface "iface0" \
        controller "port0" ipv4.method disabled
fi

# Example 21: Adding a Linux interface to a Bridge (if uplink provided)
if [[ -n "$UPLINK" ]]; then
    echo "Step 4: Creating OVS port for uplink"
    nmcli conn add type ovs-port conn.interface "port1" controller "$BRIDGE_NAME"
    
    echo "Step 5: Creating ethernet slave"
    nmcli conn add type ethernet conn.interface "$UPLINK" controller "port1"
fi

echo
echo "Done! Check with: ovs-vsctl show"