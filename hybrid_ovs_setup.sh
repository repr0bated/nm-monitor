#!/bin/bash
# Hybrid OVS Setup: Manual bridge creation + systemd-networkd IP config

set -euo pipefail

BRIDGE_NAME="vmbr0"
UPLINK="ens1"
IP_ADDR="80.209.240.244/25"
GATEWAY="80.209.240.129"

echo "🌉 HYBRID OVS BRIDGE SETUP..."
echo "Manual OVS creation + systemd-networkd IP configuration"

# 1. Stop network management
echo "📴 Stopping network services..."
sudo systemctl stop ovs-port-agent 2>/dev/null || true
sudo systemctl stop systemd-networkd 2>/dev/null || true

# 2. Clean up existing config
echo "🧹 Cleaning existing configuration..."
sudo ovs-vsctl del-br $BRIDGE_NAME 2>/dev/null || true
sudo rm -f /etc/systemd/network/*.network
sudo rm -f /etc/systemd/network/*.netdev

# 3. Create OVS bridge manually (systemd-networkd can't do this)
echo "🌉 Creating OVS bridge manually..."
sudo ovs-vsctl add-br $BRIDGE_NAME

# 4. Disable STP
echo "🚫 Disabling STP..."
sudo ovs-vsctl set bridge $BRIDGE_NAME stp_enable=false
sudo ovs-vsctl set bridge $BRIDGE_NAME other_config:disable-in-band=true

# 5. Add uplink to bridge
echo "🔗 Adding uplink to bridge..."
sudo ovs-vsctl add-port $BRIDGE_NAME $UPLINK

# 6. Create systemd-networkd config for IP (systemd-networkd CAN do this)
echo "📄 Creating IP configuration..."
sudo tee /etc/systemd/network/${BRIDGE_NAME}.network > /dev/null << NETWORK_EOF
[Match]
Name=${BRIDGE_NAME}

[Network]
DHCP=no
Address=${IP_ADDR}
Gateway=${GATEWAY}
DNS=8.8.8.8
DNS=8.8.4.4
NETWORK_EOF

# 7. Start systemd-networkd (for IP management only)
echo "▶️  Starting systemd-networkd for IP config..."
sudo systemctl enable systemd-networkd
sudo systemctl start systemd-networkd

# 8. Wait for configuration
echo "⏳ Waiting for IP configuration..."
sleep 3

# 9. Verify setup
echo ""
echo "🔍 VERIFICATION:"
echo "OVS Bridge:"
sudo ovs-vsctl show

echo ""
echo "Network status:"
networkctl status $BRIDGE_NAME 2>/dev/null || echo "Bridge not managed by networkctl"

echo ""
echo "IP configuration:"
ip addr show $BRIDGE_NAME
ip addr show $UPLINK

echo ""
echo "Connectivity test:"
ping -c 1 8.8.8.8 && echo "✅ Internet OK" || echo "❌ Internet FAILED"

echo ""
echo "✅ Hybrid OVS setup complete!"
echo "Bridge: $BRIDGE_NAME (manual OVS creation)"
echo "IP: $IP_ADDR (systemd-networkd management)"
echo "STP: Disabled"
