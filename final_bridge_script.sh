#!/bin/bash
# CAREFUL OVS BRIDGE SETUP - Safe for NoVNC

set -e  # Exit on any error

echo "🌉 CAREFUL OVS BRIDGE SETUP"
echo "==========================="

# STEP 1: Check current state
echo "📊 STEP 1: Checking current network state..."
echo "Current interfaces:"
ip addr show | grep -E "inet |^[0-9]:" | head -10
echo ""
echo "Testing connectivity:"
if ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1; then
    echo "✅ Internet connection: OK"
else
    echo "❌ Internet connection: FAILED"
fi

# STEP 2: Clean up carefully
echo ""
echo "🧹 STEP 2: Cleaning up existing configuration..."
systemctl stop systemd-networkd 2>/dev/null || echo "systemd-networkd not running"
ovs-vsctl del-br vmbr0 2>/dev/null || echo "vmbr0 not found"
rm -f /etc/systemd/network/vmbr0.network || echo "config file not found"

# STEP 3: Create OVS bridge
echo ""
echo "🌉 STEP 3: Creating OVS bridge..."
ovs-vsctl add-br vmbr0
if ! ovs-vsctl list-br | grep -q vmbr0; then
    echo "❌ Failed to create bridge"
    exit 1
fi
echo "✅ Bridge vmbr0 created"

# STEP 4: Add physical interface
echo ""
echo "🔗 STEP 4: Adding physical interface to bridge..."
ovs-vsctl add-port vmbr0 ens1
if ! ovs-vsctl list-ports vmbr0 | grep -q ens1; then
    echo "❌ Failed to add ens1 to bridge"
    exit 1
fi
echo "✅ ens1 added to vmbr0"

# STEP 5: Configure DHCP
echo ""
echo "📡 STEP 5: Configuring DHCP on bridge..."
cat > /etc/systemd/network/vmbr0.network << 'NET_EOF'
[Match]
Name=vmbr0

[Network]
DHCP=yes

[DHCP]
RouteMetric=10
NET_EOF
echo "✅ DHCP configuration created"

# STEP 6: Start network service
echo ""
echo "▶️  STEP 6: Starting network service..."
if ! systemctl start systemd-networkd; then
    echo "❌ Failed to start systemd-networkd"
    exit 1
fi
echo "✅ systemd-networkd started"

# STEP 7: Wait for DHCP
echo ""
echo "⏳ STEP 7: Waiting for DHCP lease..."
sleep 10

# STEP 8: Verify everything
echo ""
echo "🧪 STEP 8: VERIFICATION"
echo "======================"

echo "Bridge topology:"
ovs-vsctl show

echo ""
echo "IP addresses:"
ip addr show vmbr0
ip addr show ens1

echo ""
echo "Connectivity test:"
if ping -c 2 -W 3 8.8.8.8 >/dev/null 2>&1; then
    echo "✅ Internet connectivity: WORKING"
    SUCCESS=true
else
    echo "❌ Internet connectivity: FAILED"
    SUCCESS=false
fi

echo ""
if [ "$SUCCESS" = true ]; then
    echo "🎉 SUCCESS! OVS Bridge is working!"
    echo "   • vmbr0 bridge created"
    echo "   • ens1 attached to bridge"
    echo "   • DHCP obtained IP address"
    echo "   • Internet connectivity maintained"
else
    echo "⚠️  PARTIAL SUCCESS: Bridge created but no internet"
    echo "   • Check DHCP server"
    echo "   • Verify VLAN configuration"
fi

echo ""
echo "Bridge setup complete!"
