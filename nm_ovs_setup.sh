#!/bin/bash
# NetworkManager OVS Bridge Setup (replaces systemd-networkd)

set -euo pipefail

BRIDGE_NAME="vmbr0"
UPLINK="ens1"
IP_ADDR="80.209.240.244/25"
GATEWAY="80.209.240.129"

echo "🌉 CREATING NETWORKMANAGER OVS BRIDGE..."

# 1. Create OVS bridge
echo "Creating OVS bridge: $BRIDGE_NAME"
sudo ovs-vsctl add-br $BRIDGE_NAME

# 2. Disable STP (prevents connectivity delays)
echo "Disabling STP on $BRIDGE_NAME"
sudo ovs-vsctl set bridge $BRIDGE_NAME stp_enable=false
sudo ovs-vsctl set bridge $BRIDGE_NAME other_config:disable-in-band=true

# 3. Add uplink to bridge
echo "Adding $UPLINK to bridge $BRIDGE_NAME"
sudo ovs-vsctl add-port $BRIDGE_NAME $UPLINK

# 4. Create NetworkManager connection for the bridge
echo "Creating NetworkManager connection for $BRIDGE_NAME"
sudo nmcli connection add type ovs-bridge \
    conn.interface $BRIDGE_NAME \
    conn.name $BRIDGE_NAME \
    802-3-ethernet.mtu 1500

# 5. Configure IP on the bridge
echo "Configuring IP $IP_ADDR on $BRIDGE_NAME"
sudo nmcli connection modify $BRIDGE_NAME \
    ipv4.method manual \
    ipv4.addresses $IP_ADDR \
    ipv4.gateway $GATEWAY \
    ipv4.dns "8.8.8.8,8.8.4.4"

# 6. Create OVS port connection for uplink
echo "Creating OVS port for $UPLINK"
sudo nmcli connection add type ovs-port \
    conn.interface $UPLINK \
    conn.name ${UPLINK}-port \
    conn.master $BRIDGE_NAME

# 7. Create ethernet connection for uplink (slave)
echo "Creating ethernet slave connection for $UPLINK"
sudo nmcli connection add type ethernet \
    conn.interface $UPLINK \
    conn.name ${UPLINK}-slave \
    conn.master ${UPLINK}-port \
    conn.slave-type ovs-port

# 8. Bring up the bridge
echo "Bringing up $BRIDGE_NAME"
sudo nmcli connection up $BRIDGE_NAME

# 9. Verify configuration
echo "🔍 Verifying configuration..."
echo "OVS bridges:"
sudo ovs-vsctl show

echo ""
echo "NetworkManager connections:"
nmcli connection show

echo ""
echo "Interface status:"
ip addr show $BRIDGE_NAME
ip addr show $UPLINK

echo ""
echo "Testing connectivity..."
ping -c 1 8.8.8.8 && echo "✅ Internet connectivity OK" || echo "❌ Internet connectivity FAILED"

echo ""
echo "✅ NetworkManager OVS bridge setup complete!"
echo "Bridge: $BRIDGE_NAME with IP $IP_ADDR"
echo "STP: Disabled"
echo "Uplink: $UPLINK enslaved to bridge"
