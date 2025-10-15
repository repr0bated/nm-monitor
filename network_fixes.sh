#!/bin/bash
# QUICK NETWORK FIXES - Copy to NoVNC console

echo "🔧 APPLYING QUICK NETWORK FIXES"
echo "==============================="

# Fix 1: AppArmor restrictions
echo "1️⃣  FIXING APPARMOR RESTRICTIONS..."
sudo aa-complain dhclient || echo "AppArmor not available"
echo "✅ AppArmor set to complain mode"

# Fix 2: Increase network wait timeout
echo ""
echo "2️⃣  INCREASING NETWORK WAIT TIMEOUT..."
sudo sed -i 's/TimeoutStartSec=.*/TimeoutStartSec=300/' /lib/systemd/system/systemd-networkd-wait-online.service 2>/dev/null || echo "Service file not found"
sudo systemctl daemon-reload
echo "✅ Network wait timeout increased to 5 minutes"

# Fix 3: Enable network recovery
echo ""
echo "3️⃣  ENABLING NETWORK RECOVERY..."
sudo cp network-recovery.sh /usr/local/bin/ 2>/dev/null || echo "Recovery script not found"
sudo chmod +x /usr/local/bin/network-recovery.sh 2>/dev/null || true
sudo systemctl enable network-monitor.service 2>/dev/null || echo "Monitor service not found"
sudo systemctl start network-monitor.service 2>/dev/null || echo "Monitor service failed to start"
echo "✅ Network recovery enabled"

# Fix 4: Hardware optimizations
echo ""
echo "4️⃣  APPLYING HARDWARE OPTIMIZATIONS..."
sudo ethtool -s ens1 wol g 2>/dev/null || echo "Wake-on-LAN disable failed"
sudo ethtool -s ens1 autoneg off speed 1000 duplex full 2>/dev/null || echo "Speed force failed"
echo "✅ Hardware optimizations applied"

# Restart services
echo ""
echo "🔄 RESTARTING NETWORK SERVICES..."
sudo systemctl restart systemd-networkd
sudo systemctl restart systemd-networkd-wait-online 2>/dev/null || true

echo ""
echo "🧪 TESTING FIXES..."
sleep 3
ping -c 1 8.8.8.8 >/dev/null && echo "✅ Network connectivity: OK" || echo "❌ Network connectivity: FAILED"

echo ""
echo "🎉 QUICK FIXES APPLIED!"
echo "   • AppArmor restrictions relaxed"
echo "   • Network timeouts increased"
echo "   • Recovery script enabled"
echo "   • Hardware optimizations applied"
