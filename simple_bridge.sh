#!/bin/bash
# SIMPLE OVS BRIDGE CREATION - No fancy atomic stuff

echo "🌉 SIMPLE OVS BRIDGE SETUP"
echo "=========================="

# 1. Clean everything
echo "🧹 Cleaning up..."
systemctl stop systemd-networkd 2>/dev/null || true
ovs-vsctl del-br vmbr0 2>/dev/null || true
rm -f /etc/systemd/network/vmbr0.network

# 2. Create basic OVS bridge
echo "🌉 Creating OVS bridge..."
ovs-vsctl add-br vmbr0
ovs-vsctl add-port vmbr0 ens1

# 3. Configure IP on bridge (DHCP)
echo "📡 Setting up IP on bridge..."
cat > /etc/systemd/network/vmbr0.network << 'EOF'
[Match]
Name=vmbr0

[Network]
DHCP=yes
