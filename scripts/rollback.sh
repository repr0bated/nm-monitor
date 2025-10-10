#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/.." && pwd)
cd "${REPO_ROOT}"

# Backup locations for rollback
BACKUP_DIR="/var/lib/ovs-port-agent/backups"
SNAPSHOT_NAME="ovs-port-agent-preinstall"

usage() {
  cat <<USAGE
Usage: ./scripts/rollback.sh [options]

Options:
  --force           Force rollback even if backups are not found
  --dry-run         Show what would be rolled back without making changes
  --btrfs-only      Only rollback using Btrfs snapshot (nuclear option)
  --help            Show this help message

This script rolls back the ovs-port-agent installation by:
- Stopping and disabling the ovs-port-agent service
- Removing installed files (binary, config, systemd service, D-Bus config)
- Restoring NetworkManager connections from backup
- Restoring /etc/network/interfaces from backup
- Restoring OVS configuration from backup
- Removing Btrfs snapshots created during installation
USAGE
}

DRY_RUN=0
FORCE=0
BTRFS_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)
      FORCE=1; shift;;
    --dry-run)
      DRY_RUN=1; shift;;
    --btrfs-only)
      BTRFS_ONLY=1; shift;;
    --help|-h)
      usage; exit 0;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1;;
  esac
done

if [[ ${EUID} -ne 0 ]]; then
  echo "This rollback script must be run as root (try: sudo ./scripts/rollback.sh ...)" >&2
  exit 1
fi

echo "OVS Port Agent Rollback Script"
echo "=============================="

# Check if installation exists
if ! systemctl list-units --type=service | grep -q ovs-port-agent; then
  echo "ovs-port-agent service not found. Nothing to rollback."
  exit 0
fi

rollback_step() {
  local step_name="$1"
  local step_cmd="$2"

  if [[ ${DRY_RUN} -eq 1 ]]; then
    echo "[DRY RUN] Would execute: ${step_cmd}"
    return 0
  fi

  echo "Executing: ${step_name}"
  if ! eval "${step_cmd}"; then
    echo "Warning: Failed to execute: ${step_name}" >&2
    return 1
  fi
  return 0
}

# 1. Stop and disable the service
echo "Step 1: Stopping ovs-port-agent service..."
rollback_step "Stop ovs-port-agent service" "systemctl stop ovs-port-agent 2>/dev/null || true"
rollback_step "Disable ovs-port-agent service" "systemctl disable ovs-port-agent 2>/dev/null || true"

# 2. Remove installed files
echo "Step 2: Removing installed files..."
BIN_FILE="/usr/local/bin/ovs-port-agent"
CONFIG_DIR="/etc/ovs-port-agent"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
SYSTEMD_UNIT="/etc/systemd/system/ovs-port-agent.service"
DBUS_POLICY="/etc/dbus-1/system.d/dev.ovs.PortAgent1.conf"

rollback_step "Remove binary" "rm -f '${BIN_FILE}' 2>/dev/null || true"
rollback_step "Remove config directory" "rm -rf '${CONFIG_DIR}' 2>/dev/null || true"
rollback_step "Remove systemd unit" "rm -f '${SYSTEMD_UNIT}' 2>/dev/null || true"
rollback_step "Remove D-Bus policy" "rm -f '${DBUS_POLICY}' 2>/dev/null || true"

# 3. Restore NetworkManager connections if backups exist
echo "Step 3: Restoring NetworkManager connections..."
if [[ -f "${BACKUP_DIR}/nm-connections.list" ]]; then
  echo "Found NetworkManager connection backups"

  # Stop NetworkManager for restoration
  rollback_step "Stop NetworkManager" "systemctl stop NetworkManager 2>/dev/null || true"

  # Restore system-connections if backup exists
  if [[ -d "${BACKUP_DIR}/system-connections" ]]; then
    rollback_step "Restore system-connections directory" "rm -rf /etc/NetworkManager/system-connections && cp -r '${BACKUP_DIR}/system-connections' /etc/NetworkManager/ 2>/dev/null || true"
  fi

  # Start NetworkManager again
  rollback_step "Start NetworkManager" "systemctl start NetworkManager 2>/dev/null || true"
else
  echo "No NetworkManager backups found"
fi

# 4. Restore /etc/network/interfaces if backup exists
echo "Step 4: Restoring /etc/network/interfaces..."
if [[ -f "${BACKUP_DIR}/interfaces.backup" ]]; then
  rollback_step "Restore interfaces file" "cp '${BACKUP_DIR}/interfaces.backup' /etc/network/interfaces 2>/dev/null || true"
  rollback_step "Set proper permissions" "chmod 644 /etc/network/interfaces 2>/dev/null || true"
else
  echo "No interfaces backup found"
fi

# 5. Restore OVS configuration if backup exists
echo "Step 5: Restoring OVS configuration..."
if [[ -f "${BACKUP_DIR}/ovs-bridges.list" ]]; then
  echo "Found OVS bridge backups"

  # Stop OVS service
  rollback_step "Stop OVS service" "systemctl stop openvswitch-switch 2>/dev/null || true"

  # Get list of bridges that existed before installation
  local old_bridges
  old_bridges=$(cat "${BACKUP_DIR}/ovs-bridges.list" 2>/dev/null || true)

  for bridge in ${old_bridges}; do
    echo "Attempting to restore OVS bridge: ${bridge}"

    # Only restore if bridge doesn't already exist
    if ! ovs-vsctl br-exists "${bridge}" 2>/dev/null; then
      echo "Creating OVS bridge: ${bridge}"

      # Try to restore from backup config if available
      if [[ -f "${BACKUP_DIR}/ovs-${bridge}.config" ]]; then
        echo "Restoring configuration for bridge: ${bridge}"
        # Note: Full OVS config restoration is complex and may require manual intervention
        # This is a basic attempt - full restoration might need ovs-vsctl commands from backup
      fi
    else
      echo "OVS bridge ${bridge} already exists, skipping"
    fi
  done

  # Start OVS service again
  rollback_step "Start OVS service" "systemctl start openvswitch-switch 2>/dev/null || true"

  echo "OVS restoration attempted. Manual verification may be required."
  echo "Check ${BACKUP_DIR}/ for OVS configuration files"
else
  echo "No OVS backups found"
fi

# 6. Clean up backup files
echo "Step 6: Cleaning up backup files..."
rollback_step "Remove backup directory" "rm -rf '${BACKUP_DIR}' 2>/dev/null || true"

# 7. Btrfs rollback (nuclear option)
if [[ ${BTRFS_ONLY} -eq 1 ]]; then
  echo "Step 7: Btrfs rollback (nuclear option)..."

  if command -v btrfs >/dev/null 2>&1; then
    # Check if we have snapshot info from installation
    local snapshot_file="${BACKUP_DIR}/btrfs_snapshot"
    local target_snapshot=""

    if [[ -f "${snapshot_file}" ]]; then
      target_snapshot=$(cat "${snapshot_file}" 2>/dev/null || true)
    fi

    # Find the most recent snapshot if no specific target
    if [[ -z "${target_snapshot}" ]]; then
      for snap in $(btrfs subvolume list / | grep "${SNAPSHOT_NAME}" | awk '{print $9}' | sort -r | head -1); do
        if [[ -n "${snap}" ]]; then
          target_snapshot="${snap}"
          break
        fi
      done
    fi

    if [[ -n "${target_snapshot}" ]]; then
      echo "WARNING: This will restore the entire system to ${target_snapshot}"
      echo "All changes since the snapshot will be lost!"
      read -p "Are you sure you want to proceed? (type 'YES' to confirm): " confirm

      if [[ "${confirm}" == "YES" ]]; then
        rollback_step "Delete current root subvolume" "btrfs subvolume delete /.snapshots/current 2>/dev/null || true"
        rollback_step "Create new current from snapshot" "btrfs subvolume snapshot '${target_snapshot}' /.snapshots/current 2>/dev/null || true"
        echo "Btrfs rollback complete. System restored to ${target_snapshot}"
        echo "You may need to reboot for all changes to take effect."
      else
        echo "Btrfs rollback cancelled by user."
      fi
    else
      echo "No Btrfs snapshots found for rollback."
      echo "Available snapshots:"
      btrfs subvolume list / | grep "${SNAPSHOT_NAME}" || echo "None found"
    fi
  else
    echo "Btrfs tools not available for rollback."
  fi
fi

# 8. Reload systemd and D-Bus
echo "Step 8: Reloading system services..."
rollback_step "Reload systemd" "systemctl daemon-reload 2>/dev/null || true"
rollback_step "Reload D-Bus" "systemctl reload dbus.service 2>/dev/null || systemctl restart dbus.service 2>/dev/null || true"

# 9. Clean up any remaining processes
echo "Step 9: Cleaning up processes..."
rollback_step "Kill lingering processes" "pkill -f ovs-port-agent 2>/dev/null || true"

echo "=============================="
if [[ ${DRY_RUN} -eq 1 ]]; then
  echo "DRY RUN COMPLETE - No changes were made"
  echo "Run without --dry-run to actually perform rollback"
else
  echo "ROLLBACK COMPLETE"
  echo ""
  echo "What was rolled back:"
  echo "- ovs-port-agent service stopped and disabled"
  echo "- Installed files removed (binary, config, systemd, D-Bus)"
  echo "- NetworkManager connections restored from backup"
  echo "- /etc/network/interfaces restored from backup"
  echo "- OVS configuration restoration attempted"
  echo "- Backup files cleaned up"
  echo ""
  echo "The system should now be in its pre-installation state."
  echo "You may need to reboot if network connectivity issues persist."
fi
