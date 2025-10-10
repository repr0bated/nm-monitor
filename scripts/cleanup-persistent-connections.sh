#!/usr/bin/env bash
set -euo pipefail

echo "🔍 Comprehensive Connection Cleanup Script"
echo "=========================================="

# Stop NetworkManager to prevent recreation
echo "Stopping NetworkManager..."
systemctl stop NetworkManager

# Backup current connections
echo "Backing up current connections..."
nmcli -t -f NAME,UUID,TYPE,STATE connection show > /tmp/nm-connections-before.txt

# Delete all connections except essential ones
echo "Deleting all non-essential connections..."
while IFS=':' read -r conn_name uuid conn_type conn_state; do
    [[ -z "${conn_name}" ]] && continue
    
    # Keep only essential connections
    case "${conn_name}" in
        lo|docker0|virbr0|ovs-system)
            echo "  ✅ Keeping essential: ${conn_name}"
            ;;
        *)
            echo "  🗑️  Deleting: ${conn_name}"
            nmcli connection delete "${conn_name}" 2>/dev/null || true
            ;;
    esac
done < /tmp/nm-connections-before.txt

# Clean up system-connections directory
echo "Cleaning system-connections directory..."
for conn_file in /etc/NetworkManager/system-connections/*; do
    [[ -f "${conn_file}" ]] || continue
    conn_name=$(basename "${conn_file}")
    
    # Keep only essential connection files
    case "${conn_name}" in
        lo*|docker0*|virbr0*|ovs-system*)
            echo "  ✅ Keeping file: ${conn_name}"
            ;;
        *)
            echo "  🗑️  Removing file: ${conn_name}"
            rm -f "${conn_file}"
            ;;
    esac
done

# Restart NetworkManager
echo "Restarting NetworkManager..."
systemctl start NetworkManager

# Wait for NetworkManager to settle
sleep 5

# Show final state
echo "Final connection state:"
nmcli connection show

echo "✅ Cleanup complete!"
echo "💡 If connections reappear, check for:"
echo "   - Container runtimes recreating connections"
echo "   - /etc/NetworkManager/conf.d/ configuration files"
echo "   - Systemd services with network configuration"
echo "   - /etc/network/interfaces with bridge configurations"
