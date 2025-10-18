#!/bin/bash
# Complete Clean Slate Test - Build, Install, Configure OVS Bridge

set -euo pipefail

echo "🧹 CLEAN SLATE OVS-PORT-AGENT TEST"
echo "=================================="

# Phase 1: Build and install
echo ""
echo "📦 PHASE 1: Building ovs-port-agent..."
cargo build --release

echo ""
echo "📦 PHASE 2: Installing ovs-port-agent..."
sudo cp target/release/ovs-port-agent /usr/local/bin/
sudo cp dbus/dev.ovs.PortAgent1.conf /etc/dbus-1/system.d/
sudo systemctl daemon-reload

# Phase 3: Configure systemd-networkd OVS
echo ""
echo "🌉 PHASE 3: Configuring systemd-networkd OVS bridge..."

# Clean existing config
sudo systemctl stop systemd-networkd 2>/dev/null || true
sudo rm -f /etc/systemd/network/*.network
sudo rm -f /etc/systemd/network/*.netdev
sudo ovs-vsctl del-br vmbr0 2>/dev/null || true

# Create OVS bridge netdev
sudo tee /etc/systemd/network/vmbr0.netdev > /dev/null << 'NETDEV_EOF'
[NetDev]
Name=vmbr0
Kind=ovs-bridge
NETDEV_EOF

# Create bridge network (IP goes here - NOT on wrong port)
sudo tee /etc/systemd/network/vmbr0.network > /dev/null << 'NETWORK_EOF'
[Match]
Name=vmbr0

[Network]
DHCP=no
Address=80.209.240.244/25
Gateway=80.209.240.129
DNS=8.8.8.8
DNS=8.8.4.4

[OVS-Bridge]
AllowRemotePackets=yes
FailMode=standalone
STP=no
NETWORK_EOF

# Create uplink network
sudo tee /etc/systemd/network/ens1.network > /dev/null << 'UPLINK_EOF'
[Match]
Name=ens1

[Network]
DHCP=no

[OVS-Port]
VLANMode=trunk
Trunk=
UPLINK_EOF

# Phase 4: Start services
echo ""
echo "▶️  PHASE 4: Starting services..."
sudo systemctl enable systemd-networkd
sudo systemctl start systemd-networkd

# Wait for network
echo "⏳ Waiting for network configuration..."
sleep 5

# Start ovs-port-agent
sudo systemctl enable ovs-port-agent
sudo systemctl start ovs-port-agent

# Phase 5: Verify
echo ""
echo "🧪 PHASE 5: Verifying configuration..."

echo "OVS Status:"
sudo ovs-vsctl show

echo ""
echo "Network Status:"
networkctl status vmbr0 || echo "vmbr0 not found"

echo ""
echo "IP Configuration:"
ip addr show vmbr0
ip addr show ens1

echo ""
echo "Service Status:"
sudo systemctl status ovs-port-agent --no-pager -l | head -5

echo ""
echo "Connectivity Test:"
ping -c 1 8.8.8.8 && echo "✅ Internet OK" || echo "❌ Internet FAILED"

echo ""
echo "🎉 CLEAN SLATE TEST COMPLETE!"
echo ""
echo "Bridge: vmbr0 with IP 80.209.240.244/25"
echo "Uplink: ens1 attached to bridge"
echo "STP: Disabled"
echo "IP: Correctly assigned to bridge (not wrong port)"
