#!/bin/bash
# DHCP-based OVS Bridge Setup for transparent VLAN compatibility

set -euo pipefail

BRIDGE_NAME="vmbr0"
UPLINK="ens1"

echo "🌉 DHCP OVS BRIDGE SETUP (Transparent VLAN Compatible)"
echo "==================================================="

# 1. Clean existing config
echo "🧹 Cleaning existing configuration..."
sudo systemctl stop systemd-networkd 2>/dev/null || true
sudo ovs-vsctl del-br $BRIDGE_NAME 2>/dev/null || true
sudo rm -f /etc/systemd/network/*.network
sudo rm -f /etc/systemd/network/*.netdev

# 2. Create OVS bridge manually
echo "🌉 Creating OVS bridge manually..."
sudo ovs-vsctl add-br $BRIDGE_NAME
sudo ovs-vsctl set bridge $BRIDGE_NAME stp_enable=false
sudo ovs-vsctl set bridge $BRIDGE_NAME other_config:disable-in-band=true
sudo ovs-vsctl add-port $BRIDGE_NAME $UPLINK

# 3. Configure DHCP on bridge (transparent VLAN compatible)
echo "📄 Configuring DHCP on bridge..."
sudo tee /etc/systemd/network/${BRIDGE_NAME}.network > /dev/null << NETWORK_EOF
[Match]
Name=${BRIDGE_NAME}

[Network]
DHCP=yes

[DHCP]
RouteMetric=10
NETWORK_EOF

# 4. Start systemd-networkd
echo "▶️  Starting systemd-networkd with DHCP..."
sudo systemctl enable systemd-networkd
sudo systemctl start systemd-networkd

# 5. Wait for DHCP
echo "⏳ Waiting for DHCP lease..."
sleep 10

# 6. Verify
echo ""
echo "🔍 VERIFICATION:"
echo "OVS Bridge:"
sudo ovs-vsctl show

echo ""
echo "DHCP Lease:"
ip addr show $BRIDGE_NAME

echo ""
echo "Connectivity test:"
ping -c 1 8.8.8.8 && echo "✅ Internet OK" || echo "❌ Internet FAILED"

echo ""
echo "🎉 DHCP OVS bridge setup complete!"
echo "This should work with transparent VLAN configurations!"
