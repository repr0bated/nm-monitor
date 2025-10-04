#!/usr/bin/env bash
# Create OVS bridge EXACTLY as shown in NetworkManager documentation
# Examples 20 and 21 from NetworkManager.dev

set -euo pipefail

# Parameters with defaults matching the documentation
BRIDGE_NAME="${1:-bridge0}"
PORT_NAME="${2:-port0}"
IFACE_NAME="${3:-iface0}"
IP_ADDRESS="${4:-192.0.2.1/24}"
UPLINK="${5:-}"  # Optional: physical interface like eth0

echo "Creating OVS bridge following NetworkManager documentation examples EXACTLY"
echo
echo "Bridge: $BRIDGE_NAME"
echo "Port: $PORT_NAME"
echo "Interface: $IFACE_NAME"
echo "IP: $IP_ADDRESS"
[[ -n "$UPLINK" ]] && echo "Uplink: $UPLINK"
echo

# Clean up any existing connections
echo "Cleaning up any existing connections..."
for conn in $(nmcli -t -f NAME,TYPE connection show | grep "ovs-" | cut -d: -f1); do
    if [[ "$conn" == *"$BRIDGE_NAME"* ]] || [[ "$conn" == *"$PORT_NAME"* ]] || [[ "$conn" == *"$IFACE_NAME"* ]]; then
        echo "  Deleting: $conn"
        nmcli connection delete "$conn" 2>/dev/null || true
    fi
done

echo
echo "Example 20. Creating a Bridge with a single internal Interface"
echo

# Step 1: Create bridge
echo "$ nmcli conn add type ovs-bridge conn.interface $BRIDGE_NAME"
nmcli conn add type ovs-bridge conn.interface "$BRIDGE_NAME"

# Step 2: Create port
echo "$ nmcli conn add type ovs-port conn.interface $PORT_NAME controller $BRIDGE_NAME"
nmcli conn add type ovs-port conn.interface "$PORT_NAME" controller "$BRIDGE_NAME"

# Step 3: Create interface with IP
echo "$ nmcli conn add type ovs-interface port-type ovs-port conn.interface $IFACE_NAME \\"
echo "  controller $PORT_NAME ipv4.method manual ipv4.address $IP_ADDRESS"
nmcli conn add type ovs-interface port-type ovs-port conn.interface "$IFACE_NAME" \
    controller "$PORT_NAME" ipv4.method manual ipv4.address "$IP_ADDRESS"

# Optional: Add uplink
if [[ -n "$UPLINK" ]]; then
    echo
    echo "Example 21. Adding a Linux interface to a Bridge"
    echo
    
    # Create port for uplink
    echo "$ nmcli conn add type ovs-port conn.interface port1 controller $BRIDGE_NAME"
    nmcli conn add type ovs-port conn.interface port1 controller "$BRIDGE_NAME"
    
    # Add ethernet interface
    echo "$ nmcli conn add type ethernet conn.interface $UPLINK controller port1"
    nmcli conn add type ethernet conn.interface "$UPLINK" controller port1
fi

echo
echo "Done! As noted in the documentation:"
echo "\"before you add the Interface, the Bridge and Port devices appear active,"
echo "but are not configured in OVSDB yet.\""
echo
echo "Inspect results with:"
echo "$ ovs-vsctl show"