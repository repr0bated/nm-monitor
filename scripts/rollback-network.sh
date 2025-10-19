#!/usr/bin/env bash
# Rollback network configuration to pre-installation state

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BACKUP_DIR="/var/lib/ovs-port-agent/backups"

echo "=========================================="
echo " Network Configuration Rollback"
echo "=========================================="
echo ""

# Root check
if [[ ${EUID} -ne 0 ]]; then
  echo -e "${RED}ERROR: Must run as root${NC}" >&2
  exit 1
fi

# Check if backups exist
if [[ ! -d "${BACKUP_DIR}" ]]; then
  echo -e "${RED}ERROR: No backups found at ${BACKUP_DIR}${NC}" >&2
  exit 1
fi

# List available backups
echo "Available backups:"
echo ""
ls -lht "${BACKUP_DIR}" | grep "pre-install" | head -10
echo ""

# Get latest backup timestamp
LATEST_BACKUP=$(ls -t "${BACKUP_DIR}"/pre-install-networkctl-*.txt 2>/dev/null | head -1)
if [[ -z "${LATEST_BACKUP}" ]]; then
  echo -e "${RED}ERROR: No network backups found${NC}" >&2
  exit 1
fi

BACKUP_TIMESTAMP=$(basename "${LATEST_BACKUP}" | sed 's/pre-install-networkctl-\(.*\)\.txt/\1/')
echo "Latest backup: ${BACKUP_TIMESTAMP}"
echo ""

read -p "Rollback to this backup? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Rollback cancelled"
  exit 0
fi

echo "=========================================="
echo "Rolling back network configuration..."
echo "=========================================="
echo ""

# Show what we're rolling back from
echo "Current OVS bridges:"
ovs-vsctl list-br 2>/dev/null || echo "(none)"
echo ""

# Delete newly created bridges that weren't in backup
BACKUP_OVS="${BACKUP_DIR}/pre-install-ovs-${BACKUP_TIMESTAMP}.txt"
if [[ -f "${BACKUP_OVS}" ]]; then
  echo "Comparing with backup..."
  
  # Get bridges from backup
  OLD_BRIDGES=$(grep -A 100 "^Bridge" "${BACKUP_OVS}" 2>/dev/null | grep "Bridge" | awk '{print $2}' | tr -d '"' || true)
  CURRENT_BRIDGES=$(ovs-vsctl list-br 2>/dev/null || true)
  
  # Delete bridges not in backup
  for bridge in ${CURRENT_BRIDGES}; do
    if ! echo "${OLD_BRIDGES}" | grep -q "^${bridge}$"; then
      echo "  Deleting new bridge: ${bridge}"
      ovs-vsctl --if-exists del-br "${bridge}" 2>/dev/null || {
        echo -e "  ${YELLOW}Warning: Failed to delete ${bridge}${NC}"
      }
    else
      echo "  Preserving existing bridge: ${bridge}"
    fi
  done
else
  echo -e "${YELLOW}Warning: No OVS backup found, skipping bridge cleanup${NC}"
fi

# Remove systemd-networkd configs created by plugin
echo ""
echo "Cleaning up systemd-networkd configs..."
if [[ -d "/etc/systemd/network" ]]; then
  for netfile in /etc/systemd/network/10-*.network /etc/systemd/network/10-*.netdev; do
    if [[ -f "${netfile}" ]]; then
      filename=$(basename "${netfile}")
      echo "  Removing: ${filename}"
      rm -f "${netfile}"
    fi
  done
fi

# Reload networkd
echo ""
echo "Reloading systemd-networkd..."
if command -v networkctl >/dev/null 2>&1; then
  networkctl reload 2>/dev/null || {
    echo -e "${YELLOW}Warning: networkctl reload failed, trying restart${NC}"
    systemctl restart systemd-networkd 2>/dev/null || true
  }
fi

# Wait for network to settle
sleep 2

echo ""
echo "=========================================="
echo -e " ${GREEN}Rollback Complete${NC}"
echo "=========================================="
echo ""

# Show current state
echo "Current interfaces:"
ip link show | grep "^[0-9]" | awk '{print "  " $2}' | tr -d ':'
echo ""

echo "Current OVS bridges:"
ovs-vsctl list-br 2>/dev/null | awk '{print "  " $0}' || echo "  (none)"
echo ""

echo "Compare with backup:"
echo "  Pre-install: ${BACKUP_DIR}/pre-install-*-${BACKUP_TIMESTAMP}.txt"
echo ""
echo "Note: You may need to manually reconfigure your network"
echo "      Run: sudo networkctl status"
echo ""

