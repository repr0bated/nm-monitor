#!/bin/bash
# Proper systemd-networkd OVS Bridge Setup

set -euo pipefail

BRIDGE_NAME="vmbr0"
UPLINK="ens1"
IP_ADDR="80.209.240.244/25"
GATEWAY="80.209.240.129"

echo "🌉 CREATING PROPER SYSTEMD-NETWORKD OVS BRIDGE..."

# 1. Stop any existing network management
echo "📴 Stopping network management services..."
sudo systemctl stop ovs-port-agent 2>/dev/null || true
sudo systemctl stop systemd-networkd 2>/dev/null || true

# 2. Clean up existing OVS config
echo "🧹 Cleaning existing OVS configuration..."
sudo ovs-vsctl del-br $BRIDGE_NAME 2>/dev/null || true
sudo ovs-vsctl del-br ovsbr0 2>/dev/null || true

# 3. Remove existing network files
echo "🗑️  Removing existing network configuration..."
sudo rm -f /etc/systemd/network/*.network
sudo rm -f /etc/systemd/network/*.netdev

# 4. Create OVS bridge netdev file
echo "📄 Creating OVS bridge netdev file..."
sudo tee /etc/systemd/network/${BRIDGE_NAME}.netdev > /dev/null << NETDEV_EOF
[NetDev]
Name=${BRIDGE_NAME}
Kind=ovs-bridge
NETDEV_EOF

# 5. Create bridge network file (IP goes here - correct location)
echo "📄 Creating bridge network file..."
sudo tee /etc/systemd/network/${BRIDGE_NAME}.network > /dev/null << NETWORK_EOF
[Match]
Name=${BRIDGE_NAME}

[Network]
DHCP=no
Address=${IP_ADDR}
Gateway=${GATEWAY}
DNS=8.8.8.8
DNS=8.8.4.4

[OVS-Bridge]
AllowRemotePackets=yes
FailMode=standalone
STP=no
NETWORK_EOF

# 6. Create uplink network file
echo "📄 Creating uplink network file..."
sudo tee /etc/systemd/network/${UPLINK}.network > /dev/null << UPLINK_EOF
[Match]
Name=${UPLINK}

[Network]
DHCP=no

[OVS-Port]
VLANMode=trunk
Trunk=
UPLINK_EOF

# 7. Start systemd-networkd
echo "▶️  Starting systemd-networkd..."
sudo systemctl enable systemd-networkd
sudo systemctl start systemd-networkd

# 8. Wait for network to configure
echo "⏳ Waiting for network configuration..."
sleep 5

# 9. Verify OVS bridge
echo "🔍 Verifying OVS bridge..."
sudo ovs-vsctl show

echo ""
echo "🔍 Verifying network configuration..."
networkctl status

echo ""
echo "🔍 Verifying IP configuration..."
ip addr show $BRIDGE_NAME
ip addr show $UPLINK

echo ""
echo "🧪 Testing connectivity..."
ping -c 1 8.8.8.8 && echo "✅ Internet connectivity OK" || echo "❌ Internet connectivity FAILED"

echo ""
echo "✅ systemd-networkd OVS bridge setup complete!"
echo "Bridge: $BRIDGE_NAME with IP $IP_ADDR"
echo "STP: Disabled"
echo "Uplink: $UPLINK attached to bridge"
