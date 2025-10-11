#!/bin/bash

# ============================================================================
# OVSBRO FIX SCRIPT - Makes ovsbr0 operational with IP addressing
# Run this script on the target server via noVNC or SSH
# ============================================================================

echo "==========================================="
echo "OVSBRO FIX SCRIPT - Starting..."
echo "==========================================="

# STEP 1: Enable IP addressing (fixes connectivity issue)
echo "1. Enabling IP addressing for ovsbr0..."
sudo nmcli connection modify ovsbr0 ipv4.method auto ipv6.method auto
sudo nmcli connection modify ovsbr0_if ipv4.method auto ipv6.method auto
echo "   ✅ IP methods enabled"

# STEP 2: Fix Proxmox interface name (underscores not dashes)
echo "2. Fixing interface name for Proxmox compatibility..."
sudo nmcli connection modify ovsbr0_if connection.id "ovsbr0_if" connection.interface-name "ovsbr0_if"
echo "   ✅ Interface name fixed (ovsbr0_if)"

# STEP 3: Clean interfaces file conflicts
echo "3. Cleaning /etc/network/interfaces file conflicts..."
sudo sed -i 's/^auto ovsbr0/#auto ovsbr0/;s/^iface ovsbr0/#iface ovsbr0/' /etc/network/interfaces
echo "   ✅ Interfaces file cleaned"

# STEP 4: Restart NetworkManager
echo "4. Reloading NetworkManager configuration..."
sudo nmcli connection reload
sudo systemctl reload NetworkManager
sleep 2
echo "   ✅ NetworkManager reloaded"

# STEP 5: Activate ovsbr0 bridge
echo "5. Activating ovsbr0 bridge..."
sudo nmcli connection up ovsbr0
sudo nmcli connection up ovsbr0_if
echo "   ✅ ovsbr0 activated"

echo ""
echo "==========================================="
echo "VERIFICATION - Checking results..."
echo "==========================================="

# VERIFICATION CHECKS
echo "Bridge Status:"
sudo ovs-vsctl show | head -5

echo ""
echo "Device Status:"
sudo nmcli device status | grep ovs

echo ""
echo "IP Addresses:"
ip addr show | grep ovsbr0

echo ""
echo "Connectivity Test:"
if ping -c 2 8.8.8.8 >/dev/null 2>&1; then
    echo "✅ INTERNET CONNECTIVITY RESTORED!"
else
    echo "❌ INTERNET CONNECTIVITY NOT WORKING"
fi

echo ""
echo "==========================================="
echo "RESULTS SUMMARY"
echo "==========================================="

# FINAL STATUS
OVS_STATUS=$(sudo ovs-vsctl show | grep -c "Bridge ovsbr0" || echo "0")
IP_STATUS=$(ip addr show ovsbr0_if 2>/dev/null | grep -c "inet " || echo "0")
CONNECTIVITY_STATUS=$(ping -c 1 8.8.8.8 >/dev/null 2>&1 && echo "1" || echo "0")

if [ "$OVS_STATUS" -gt 0 ] && [ "$IP_STATUS" -gt 0 ] && [ "$CONNECTIVITY_STATUS" -eq 1 ]; then
    echo "🎉 SUCCESS! ovsbr0 is fully operational!"
    echo "🔄 Ready to add ports incrementally"
else
    echo "⚠️  PARTIAL SUCCESS or issues detected:"
    echo "   Bridge: $([ "$OVS_STATUS" -gt 0 ] && echo "✅" || echo "❌")"
    echo "   IP: $([ "$IP_STATUS" -gt 0 ] && echo "✅" || echo "❌")"
    echo "   Connectivity: $([ "$CONNECTIVITY_STATUS" -eq 1 ] && echo "✅" || echo "❌")"
    echo ""
    echo "If failed, try direct IP setup:"
    echo "sudo ip addr add 80.209.242.196/25 dev ovsbr0_if"
    echo "sudo ip route add default via 80.209.242.129 dev ovsbr0_if"
    echo "sudo ip link set dev ovsbr0_if up"
fi

echo "==========================================="
echo "SCRIPT COMPLETE"
echo "==========================================="
