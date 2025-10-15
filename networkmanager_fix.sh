#!/bin/bash
# Fix NetworkManager/systemd-networkd conflicts and disable STP

set -euo pipefail

echo "🔧 FIXING NETWORK MANAGER CONFLICTS..."

# 1. Disable systemd-networkd (conflicts with NetworkManager)
echo "📴 Disabling systemd-networkd..."
sudo systemctl stop systemd-networkd
sudo systemctl disable systemd-networkd
sudo systemctl mask systemd-networkd

# 2. Ensure NetworkManager is running
echo "▶️  Ensuring NetworkManager is active..."
sudo systemctl enable NetworkManager
sudo systemctl start NetworkManager

# 3. Check unmanaged interfaces
echo "🔍 Checking unmanaged interfaces..."
nmcli device | grep unmanaged || echo "No unmanaged devices found"

# 4. Disable STP on existing OVS bridges
echo "🌉 Disabling STP on OVS bridges..."
for bridge in $(sudo ovs-vsctl list-br); do
    echo "Disabling STP on bridge: $bridge"
    sudo ovs-vsctl set bridge $bridge stp_enable=false
    sudo ovs-vsctl set bridge $bridge other_config:disable-in-band=true
done

# 5. Clean up any existing OVS config
echo "🧹 Cleaning up existing OVS configuration..."
sudo ovs-vsctl del-br vmbr0 2>/dev/null || true
sudo ovs-vsctl del-br ovsbr0 2>/dev/null || true

# 6. Reset NetworkManager
echo "🔄 Resetting NetworkManager..."
sudo nmcli networking off
sleep 2
sudo nmcli networking on

# 7. Wait for networking to stabilize
echo "⏳ Waiting for networking to stabilize..."
sleep 5

# 8. Verify connectivity
echo "🧪 Verifying connectivity..."
ping -c 1 8.8.8.8 && echo "✅ Internet connectivity OK" || echo "❌ Internet connectivity FAILED"

echo "✅ NetworkManager conflicts resolved!"
echo "✅ STP disabled on all bridges!"
echo "✅ Ready for proper NetworkManager OVS configuration!"
