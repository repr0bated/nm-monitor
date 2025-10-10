#!/usr/bin/env bash
set -euo pipefail

echo "🔍 Network Connection Diagnostic Script"
echo "======================================="

echo "1. Current NetworkManager Connections:"
echo "-------------------------------------"
nmcli connection show

echo ""
echo "2. NetworkManager System Connections Directory:"
echo "-----------------------------------------------"
if [[ -d "/etc/NetworkManager/system-connections" ]]; then
    echo "Files in /etc/NetworkManager/system-connections/:"
    ls -la /etc/NetworkManager/system-connections/
else
    echo "Directory /etc/NetworkManager/system-connections/ does not exist"
fi

echo ""
echo "3. NetworkManager Configuration:"
echo "-------------------------------"
if [[ -f "/etc/NetworkManager/NetworkManager.conf" ]]; then
    echo "NetworkManager.conf:"
    cat /etc/NetworkManager/NetworkManager.conf
fi

echo ""
echo "4. NetworkManager Conf.d Directory:"
echo "-----------------------------------"
if [[ -d "/etc/NetworkManager/conf.d" ]]; then
    echo "Files in /etc/NetworkManager/conf.d/:"
    ls -la /etc/NetworkManager/conf.d/
    echo ""
    echo "Contents of configuration files:"
    for file in /etc/NetworkManager/conf.d/*.conf; do
        [[ -f "$file" ]] || continue
        echo "=== $file ==="
        cat "$file"
        echo ""
    done
else
    echo "Directory /etc/NetworkManager/conf.d/ does not exist"
fi

echo ""
echo "5. Systemd-Networkd Configuration:"
echo "----------------------------------"
if [[ -d "/etc/systemd/network" ]]; then
    echo "Files in /etc/systemd/network/:"
    ls -la /etc/systemd/network/
else
    echo "Directory /etc/systemd/network/ does not exist"
fi

echo ""
echo "6. /etc/network/interfaces:"
echo "--------------------------"
if [[ -f "/etc/network/interfaces" ]]; then
    echo "Contents of /etc/network/interfaces:"
    cat /etc/network/interfaces
else
    echo "/etc/network/interfaces does not exist"
fi

echo ""
echo "7. Running Processes (Network Related):"
echo "--------------------------------------"
ps aux | grep -E "(nm|network|container|docker|lxc|podman|libvirt)" | grep -v grep

echo ""
echo "8. Systemd Services (Network Related):"
echo "-------------------------------------"
systemctl list-units | grep -E "(nm|network|container|docker|lxc|podman|libvirt|ovs)" | head -10

echo ""
echo "9. OVS Bridges:"
echo "---------------"
if command -v ovs-vsctl >/dev/null 2>&1; then
    ovs-vsctl list-br 2>/dev/null || echo "OVS not available"
else
    echo "ovs-vsctl not found"
fi

echo ""
echo "10. Container Runtimes:"
echo "-----------------------"
systemctl list-units | grep -E "(docker|lxc|containerd|podman)" | head -5

echo ""
echo "======================================="
echo "🔍 Diagnostic Complete"
echo ""
echo "💡 If connections keep reappearing, check:"
echo "   - Container runtimes (Docker, LXC, Podman)"
echo "   - /etc/NetworkManager/conf.d/ configuration files"
echo "   - Systemd services that might recreate connections"
echo "   - /etc/network/interfaces bridge configurations"
