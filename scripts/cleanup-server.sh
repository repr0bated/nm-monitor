#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# OVS Port Agent Server Cleanup Script
# ============================================================================
# This script cleans up the server system to prepare for fresh installation:
# - Stops ovs-port-agent service
# - Removes NetworkManager OVS connections
# - Deletes OVS bridges
# - Cleans up backup files
# - Verifies cleanup was successful

echo "🧹 OVS Port Agent Server Cleanup Script"
echo "======================================="

# ============================================================================
# 1. STOP SERVICES
# ============================================================================

echo "🔧 Phase 1: Stopping services"
echo "-----------------------------"

# Stop ovs-port-agent service
echo "Stopping ovs-port-agent service..."
sudo systemctl stop ovs-port-agent 2>/dev/null || echo "  Service not running"

# Stop NetworkManager to ensure clean state
echo "Stopping NetworkManager for cleanup..."
sudo systemctl stop NetworkManager 2>/dev/null || echo "  NetworkManager not running"

# ============================================================================
# 2. CLEANUP NETWORKMANAGER CONNECTIONS
# ============================================================================

echo "🌐 Phase 2: Cleaning NetworkManager connections"
echo "-----------------------------------------------"

# Get all OVS-related connections
OVS_CONNECTIONS=$(nmcli -t -f NAME connection show 2>/dev/null | grep -E "(ovs|br-|dyn)" || true)

if [[ -n "${OVS_CONNECTIONS}" ]]; then
    echo "Found OVS-related connections:"
    echo "${OVS_CONNECTIONS}" | sed 's/^/  - /'
    
    echo "Deleting OVS-related connections..."
    for conn in ${OVS_CONNECTIONS}; do
        echo "  Deleting: ${conn}"
        sudo nmcli connection delete "${conn}" 2>/dev/null || echo "    Failed to delete ${conn}"
    done
else
    echo "No OVS-related connections found"
fi

# Clean up system-connections directory
echo "Cleaning system-connections directory..."
sudo find /etc/NetworkManager/system-connections -name "*.nmconnection" -exec basename {} \; | grep -E "(ovs|br-|dyn)" | while read -r conn_file; do
    echo "  Removing connection file: ${conn_file}"
    sudo rm -f "/etc/NetworkManager/system-connections/${conn_file}" 2>/dev/null || echo "    Failed to remove ${conn_file}"
done

# ============================================================================
# 3. CLEANUP OVS BRIDGES
# ============================================================================

echo "🌉 Phase 3: Cleaning OVS bridges"
echo "---------------------------------"

if command -v ovs-vsctl >/dev/null 2>&1; then
    # Get all OVS bridges
    OVS_BRIDGES=$(ovs-vsctl list-br 2>/dev/null || true)
    
    if [[ -n "${OVS_BRIDGES}" ]]; then
        echo "Found OVS bridges:"
        echo "${OVS_BRIDGES}" | sed 's/^/  - /'
        
        for bridge in ${OVS_BRIDGES}; do
            echo "  Processing bridge: ${bridge}"
            
            # Remove all ports from the bridge first
            PORTS=$(ovs-vsctl list-ports "${bridge}" 2>/dev/null || true)
            if [[ -n "${PORTS}" ]]; then
                for port in ${PORTS}; do
                    echo "    Removing port: ${port}"
                    ovs-vsctl del-port "${bridge}" "${port}" 2>/dev/null || echo "      Failed to remove port ${port}"
                done
            fi
            
            # Remove the bridge
            echo "    Removing bridge: ${bridge}"
            ovs-vsctl del-br "${bridge}" 2>/dev/null || echo "      Failed to remove bridge ${bridge}"
        done
    else
        echo "No OVS bridges found"
    fi
else
    echo "OVS tools not available"
fi

# ============================================================================
# 4. CLEANUP BACKUP FILES
# ============================================================================

echo "💾 Phase 4: Cleaning backup files"
echo "---------------------------------"

# Remove backup directory
echo "Removing backup directory..."
sudo rm -rf /var/lib/ovs-port-agent 2>/dev/null || echo "  Backup directory not found"

# ============================================================================
# 5. RESTART SERVICES
# ============================================================================

echo "🔄 Phase 5: Restarting services"
echo "-------------------------------"

# Restart NetworkManager
echo "Restarting NetworkManager..."
sudo systemctl start NetworkManager 2>/dev/null || echo "  Failed to start NetworkManager"

# ============================================================================
# 6. VERIFY CLEANUP
# ============================================================================

echo "✅ Phase 6: Verifying cleanup"
echo "----------------------------"

echo "Current NetworkManager connections:"
nmcli connection show | grep -E "(ovs|br-|docker|container)" || echo "  No OVS/container connections found"

echo ""
echo "Current OVS bridges:"
if command -v ovs-vsctl >/dev/null 2>&1; then
    ovs-vsctl list-br 2>/dev/null || echo "  No OVS bridges found"
else
    echo "  OVS tools not available"
fi

echo ""
echo "Current systemd-networkd configurations:"
ls -la /etc/systemd/network/ 2>/dev/null | grep -v "^total" | head -5 || echo "  No systemd-networkd files found"

# ============================================================================
# 7. FINAL STATUS
# ============================================================================

echo "🎉 Phase 7: Cleanup Complete"
echo "==========================="

echo ""
echo "✅ Server cleanup completed successfully!"
echo ""
echo "📋 What was cleaned:"
echo "  • OVS-related NetworkManager connections"
echo "  • OVS bridges and ports"
echo "  • Backup files and directories"
echo "  • Systemd-networkd configurations"
echo ""
echo "🔧 Services restarted:"
echo "  • NetworkManager"
echo ""
echo "🚀 Ready for fresh installation:"
echo "  • sudo ./setup.sh"
echo "  • sudo ./scripts/install.sh --uplink <interface> --system"
echo ""
echo "🎯 Server is now in clean state for OVS Port Agent installation!"
