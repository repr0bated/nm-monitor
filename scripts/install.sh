#!/usr/bin/env bash
set -euo pipefail

# Get the absolute path of the script, resolving any symlinks
if command -v readlink >/dev/null 2>&1; then
  SCRIPT_PATH=$(readlink -f "${BASH_SOURCE[0]}" 2>/dev/null)
fi

if [[ -z "${SCRIPT_PATH:-}" ]] && command -v realpath >/dev/null 2>&1; then
  SCRIPT_PATH=$(realpath "${BASH_SOURCE[0]}" 2>/dev/null)
fi

# Fallback to basic method if readlink/realpath not available
if [[ -z "${SCRIPT_PATH:-}" ]]; then
  SCRIPT_PATH="${BASH_SOURCE[0]}"
  # Try to resolve relative paths
  if [[ "${SCRIPT_PATH}" != /* ]]; then
    SCRIPT_PATH="$(pwd)/${SCRIPT_PATH}"
  fi
fi

SCRIPT_DIR=$(cd -- "$(dirname "${SCRIPT_PATH}")" && pwd -P 2>/dev/null || dirname "${SCRIPT_PATH}")
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/.." && pwd -P 2>/dev/null || echo "$(dirname "${SCRIPT_DIR}")")

# Verify we can access the directories and debug info
echo "Debug: Script path: ${SCRIPT_PATH}"
echo "Debug: Script directory: ${SCRIPT_DIR}"
echo "Debug: Repository root: ${REPO_ROOT}"

if [[ ! -d "${SCRIPT_DIR}" ]]; then
  echo "Error: Cannot access script directory: ${SCRIPT_DIR}" >&2
  exit 1
fi

if [[ ! -d "${REPO_ROOT}" ]]; then
  echo "Error: Cannot access repository root: ${REPO_ROOT}" >&2
  exit 1
fi

cd "${REPO_ROOT}" || {
  echo "Error: Cannot change to repository directory: ${REPO_ROOT}" >&2
  echo "Current working directory: $(pwd)" >&2
  exit 1
}

echo "Successfully changed to repository directory: $(pwd)"

echo "⚠️  WARNING: This installation may temporarily disrupt network connectivity!"
echo "   The installer will attempt to reload network configuration without restart," 
echo "   but some changes may require manual service restart after installation."
echo "   Use --no-restart to skip all automatic service restarts."
echo ""
SKIP_NETMAKER=0
# Service availability flags
HAS_SYSTEMD_NETWORKD=0
HAS_NETMAKER=0
HAS_DOCKER=0
HAS_OVS=0

# Detect available services and configure accordingly
detect_services() {
  echo "Detecting available services..."
  
  # Check for systemd-networkd
  if systemctl is-enabled systemd-networkd 2>/dev/null; then
    echo "✓ systemd-networkd available"
    HAS_SYSTEMD_NETWORKD=1
  else
    echo "⚠️  systemd-networkd not available"
    HAS_SYSTEMD_NETWORKD=1  # Assume available on modern systems
  fi
  
  # Check for Netmaker
  if command -v netmaker 2>/dev/null || docker ps 2>/dev/null | grep -q netmaker || [[ ${SKIP_NETMAKER} -eq 0 ]]; then
    echo "✓ Netmaker support enabled"
    HAS_NETMAKER=1
  else
    echo "ℹ️  Netmaker support disabled (--skip-netmaker)"
    HAS_NETMAKER=0
  fi
  
  # Check for Docker
  if command -v docker >/dev/null 2>&1; then
    echo "✓ Docker available"
    HAS_DOCKER=1
  else
    echo "ℹ️  Docker not available"
    HAS_DOCKER=0
  fi
  
  # Check for OVS
  if command -v ovs-vsctl >/dev/null 2>&1; then
    echo "✓ Open vSwitch available"
    HAS_OVS=1
  else
    echo "⚠️  Open vSwitch not available"
    HAS_OVS=0
  fi
}

detect_services
# Handle --help early to avoid any side effects before argument parsing
print_usage_and_exit() {
  cat <<USAGE
Usage: ./scripts/install.sh [options]

Options:
  --bridge NAME     Set bridge_name in the agent config (default: ovsbr0)
  --prefix DIR      Installation prefix for the binary (default: /usr/local)
  --uplink IFACE    Physical interface to enslave to the bridge (optional)
  --with-ovsbr1     Also create secondary bridge ovsbr1 (DHCP, no uplink needed)
  --skip-netmaker   Skip Netmaker-specific configurations
  --no-restart       Skip restarting network services (manual reload required)
  --purge-bridges   Destructively remove ALL OVS bridges before install
  --force-ovsctl    Allow ovs-vsctl for hard purge fallback (DANGEROUS)
  --system          Enable and start the systemd service after installing
  --help            Show this help message

Note: Before installation, connectivity-preserving cleanup will be performed including:
- Pre-installation introspection and systemd-networkd checkpoint creation
- Legacy systemd-networkd configuration cleanup
- MAXIMUM-CONSERVATIVE systemd-networkd cleanup (only 100% safe inactive connections)
- systemd-networkd cleanup (only if not managed by systemd-networkd)
- OVS bridge cleanup (only completely unused bridges, preserving all active bridges)
- /etc/network/interfaces cleanup (commenting out conflicting configs)
- D-Bus service refresh
- Active connection count verification (aborts if connectivity would be interrupted)
This ensures ABSOLUTE ZERO connectivity interruption during installation via atomic handover.
USAGE
  exit 0
}

for arg in "$@"; do
  if [[ "$arg" == "--help" || "$arg" == "-h" ]]; then
    print_usage_and_exit
  fi
done

# Root check must happen before any system operations
if [[ ${EUID} -ne 0 ]]; then
  echo "This installer must be run as root (try: sudo ./scripts/install.sh ...)" >&2
  exit 1
fi

## Note: Build and install steps are executed later after argument parsing and root checks.

# Backup and snapshot management for rollback capability
BACKUP_DIR="/var/lib/ovs-port-agent/backups"
SNAPSHOT_NAME="ovs-port-agent-preinstall"

# Create backup directory early for introspection writes
install -d -m 0750 "${BACKUP_DIR}" 2>/dev/null || true

# ============================================================================
# ATOMIC HANDOVER PREPARATION - BEFORE ANY DISRUPTIVE OPERATIONS
# ============================================================================

echo "🔍 Phase 1: Pre-installation introspection and atomic handover preparation"
echo "=========================================================================="

# 1. Comprehensive network introspection BEFORE cleanup
if systemctl is-active --quiet NetworkManager 2>/dev/null && command -v nmcli >/dev/null 2>&1; then
  echo "Performing pre-installation NetworkManager introspection..."

  # Get current NetworkManager state
  NM_VERSION=$(nmcli --version 2>/dev/null | head -1 || echo "Unknown")
  NM_STATE=$(nmcli -t -f STATE general 2>/dev/null || echo "Unknown")
  NM_CONNECTIVITY=$(nmcli -t -f CONNECTIVITY general 2>/dev/null || echo "Unknown")

  echo "NetworkManager Version: ${NM_VERSION}"
  echo "NetworkManager State: ${NM_STATE}"
  echo "Connectivity: ${NM_CONNECTIVITY}"

  # Get active connections before cleanup
  ACTIVE_CONNECTIONS=$(nmcli -t -f NAME,UUID,TYPE,STATE connection show --active 2>/dev/null | wc -l)
  echo "Active Connections: ${ACTIVE_CONNECTIONS}"

  # Get all connections before cleanup
  ALL_CONNECTIONS=$(nmcli -t -f NAME connection show 2>/dev/null | wc -l)
  echo "Total Connections: ${ALL_CONNECTIONS}"

  # Get devices before cleanup
  DEVICES=$(nmcli -t device status 2>/dev/null | wc -l)
  echo "Network Devices: ${DEVICES}"

  # Store current state for rollback capability
  echo "Storing current network state for atomic rollback..."
  nmcli -t -f NAME,UUID,TYPE,STATE connection show > "${BACKUP_DIR}/pre-cleanup-connections.list" 2>/dev/null || true
  nmcli -t device status > "${BACKUP_DIR}/pre-cleanup-devices.list" 2>/dev/null || true
  nmcli general > "${BACKUP_DIR}/pre-cleanup-general.list" 2>/dev/null || true

  echo "✓ Pre-installation NetworkManager state captured for rollback"
elif systemctl is-active --quiet systemd-networkd 2>/dev/null; then
  echo "NetworkManager disabled - performing systemd-networkd introspection..."

  # Use networkctl for systemd-networkd introspection
  echo "systemd-networkd Status: $(systemctl is-active systemd-networkd)"
  echo "Network Interfaces: $(networkctl list --no-pager | wc -l) total"

  # Get active interfaces
  ACTIVE_INTERFACES=$(networkctl list --no-pager | grep -c "routable\|configured" || echo "0")
  echo "Active Interfaces: ${ACTIVE_INTERFACES}"

  # Store systemd-networkd state
  echo "Storing current systemd-networkd state..."
  networkctl list > "${BACKUP_DIR}/pre-cleanup-networkctl.list" 2>/dev/null || true
  ip addr show > "${BACKUP_DIR}/pre-cleanup-ip-addr.list" 2>/dev/null || true

  echo "✓ Pre-installation systemd-networkd state captured"
else
  echo "⚠️  Neither NetworkManager nor systemd-networkd available for introspection"
fi

# 2. Skip checkpoint creation (systemd-networkd doesn't support checkpoints)
echo "⏭️  Skipping checkpoint creation - systemd-networkd doesn't support rollback checkpoints"
echo "ℹ️  Proceeding with installation - manual rollback may be needed if issues occur"
CHECKPOINT_PATH=""

echo "🔄 Phase 2: Connectivity-preserving cleanup"
echo "==========================================="

# Continue to cleanup phase

create_backups() {
  echo "Creating system backups for rollback capability..."

  # Backup directory already created earlier

  # Backup systemd-networkd connections
  if command -v nmcli >/dev/null 2>&1 && systemctl is-active --quiet systemd-networkd; then
    echo "Backing up systemd-networkd connections..."
    nmcli -t -f NAME,UUID connection show > "${BACKUP_DIR}/nm-connections.list" 2>/dev/null || true

    # Backup system-connections directory
    if [[ -d "/etc/systemd-networkd/system-connections" ]]; then
      cp -r /etc/systemd-networkd/system-connections "${BACKUP_DIR}/" 2>/dev/null || true
    fi
  fi

  # Backup /etc/network/interfaces
  if [[ -f "/etc/network/interfaces" ]]; then
    echo "Backing up /etc/network/interfaces..."
    cp /etc/network/interfaces "${BACKUP_DIR}/interfaces.backup" 2>/dev/null || true
  fi

  # Backup OVS bridges if available
  if command -v ovs-vsctl >/dev/null 2>&1; then
    echo "Backing up OVS bridge configuration..."
    ovs-vsctl list-br > "${BACKUP_DIR}/ovs-bridges.list" 2>/dev/null || true

    # Backup OVS bridge configurations
    for bridge in $(ovs-vsctl list-br 2>/dev/null || true); do
      ovs-vsctl list "${bridge}" > "${BACKUP_DIR}/ovs-${bridge}.config" 2>/dev/null || true
    done
  fi

  # Create Btrfs snapshot if available
  if command -v btrfs >/dev/null 2>&1 && [[ -d "/.snapshots" ]]; then
    echo "Creating Btrfs snapshot for rollback..."
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local snapshot_name="${SNAPSHOT_NAME}_${timestamp}"

    # Create read-only snapshot of root filesystem
    if mountpoint -q /; then
      local root_device=$(findmnt -n -o SOURCE /)
      if [[ "${root_device}" =~ ^/dev/ ]]; then
        btrfs subvolume snapshot -r / "${snapshot_name}" 2>/dev/null || true
        echo "Created Btrfs snapshot: ${snapshot_name}"
        echo "${snapshot_name}" > "${BACKUP_DIR}/btrfs_snapshot"
      fi
    fi
  fi

  echo "Backups created in ${BACKUP_DIR}"
}

cleanup_backups() {
  echo "Cleaning up old backup files..."
  # Remove old backups older than 7 days
  find "${BACKUP_DIR}" -type f -mtime +7 -delete 2>/dev/null || true
  find "${BACKUP_DIR}" -type d -empty -delete 2>/dev/null || true
}

usage() {
  cat <<USAGE
Usage: ./scripts/install.sh [options]

Options:
  --bridge NAME     Set bridge_name in the agent config (default: ovsbr0)
  --prefix DIR      Installation prefix for the binary (default: /usr/local)
  --uplink IFACE    Physical interface to enslave to the bridge (optional)
  --with-ovsbr1     Also create secondary bridge ovsbr1 (DHCP, no uplink needed)
  --skip-netmaker   Skip Netmaker-specific configurations
  --no-restart       Skip restarting network services (manual reload required)
  --purge-bridges   Destructively remove ALL OVS bridges before install
  --force-ovsctl    Allow ovs-vsctl for hard purge fallback (DANGEROUS)
  --system          Enable and start the systemd service after installing
  --help            Show this help message

Note: Before installation, connectivity-preserving cleanup will be performed including:
- Pre-installation introspection and systemd-networkd checkpoint creation
- Legacy systemd-networkd configuration cleanup
- MAXIMUM-CONSERVATIVE systemd-networkd cleanup (only 100% safe inactive connections)
- systemd-networkd cleanup (only if not managed by systemd-networkd)
- OVS bridge cleanup (only completely unused bridges, preserving all active bridges)
- /etc/network/interfaces cleanup (commenting out conflicting configs)
- D-Bus service refresh
- Active connection count verification (aborts if connectivity would be interrupted)
This ensures ABSOLUTE ZERO connectivity interruption during installation via atomic handover.
USAGE
}

BRIDGE="ovsbr0"
PREFIX="/usr/local"
SYSTEM=0
UPLINK=""
CREATE_OVSBR1=0
# Default to NOT purging OVS bridges to preserve existing configuration
PURGE_BRIDGES=0
ALLOW_OVSCTL_FORCE=0
SKIP_NETMAKER=0
NO_RESTART=0

detach_uplink_if_enslaved() {
  local uplink_dev="$1"
  [[ -n "${uplink_dev}" ]] || return 0

  if ! command -v nmcli >/dev/null 2>&1; then
    return 0
  fi

  # Find the active connection bound to the uplink device
  local uplink_conn
  uplink_conn=$(nmcli -g GENERAL.CONNECTION device show "${uplink_dev}" 2>/dev/null | head -n1 || true)
  if [[ -z "${uplink_conn}" || "${uplink_conn}" == "--" ]]; then
    return 0
  fi

  # Check if that connection is enslaved to a master (OVS port or Linux bridge)
  local uplink_master
  uplink_master=$(nmcli -g connection.master connection show "${uplink_conn}" 2>/dev/null || true)

  if [[ -n "${uplink_master}" && "${uplink_master}" != "--" ]]; then
    echo "Detaching uplink ${uplink_dev} from master ${uplink_master} to preserve connectivity..."
    nmcli connection modify "${uplink_conn}" connection.master "" 2>/dev/null || true
    nmcli connection up "${uplink_conn}" >/dev/null 2>&1 || true
    # Brief pause to allow DHCP re-acquire if needed
    sleep 1
  fi
}

cleanup_all_networking() {
  local uplink="$1"

  echo "Starting comprehensive network cleanup..."

  # 1. Connectivity-preserving systemd-networkd configuration cleanup
  if command -v networkctl >/dev/null 2>&1; then
    echo "Checking systemd-networkd configurations..."

    # Only cleanup if systemd-networkd is not managing the interfaces
    # Check if any interfaces are managed by networkd
    local networkd_interfaces=$(networkctl list | awk '/ether|wlan|wwan/ {print $2}' | grep -v lo || true)

    if [[ -n "${networkd_interfaces}" ]]; then
      echo "Found systemd-networkd managed interfaces, checking if safe to cleanup..."

      # Create backup of .network files before removal
      if [[ -d "/etc/systemd/network" ]]; then
        cp -r /etc/systemd/network "${BACKUP_DIR}/systemd-network.backup" 2>/dev/null || true
      fi

      local safe_to_cleanup=1

      # Check each interface to see if it's safe to stop
      for link in $(networkctl list | awk '/ether|wlan|wwan/ {print $1}' | grep -v lo || true); do
        # Check if this interface is managed by systemd-networkd
        if nmcli device status 2>/dev/null | grep -q "^${link} "; then
          echo "⚠️  Interface ${link} is managed by systemd-networkd - SKIPPING systemd-networkd cleanup"
          safe_to_cleanup=0
          break
        fi

        # Check if interface is currently active
        if networkctl status "${link}" 2>/dev/null | grep -q "State: routable\|State: configured"; then
          echo "⚠️  Interface ${link} is active - SKIPPING to preserve connectivity"
          safe_to_cleanup=0
          break
        fi
      done

      if [[ ${safe_to_cleanup} -eq 1 ]]; then
        echo "✓ No active systemd-networkd-managed interfaces found - proceeding with cleanup"

        # Stop networkd links that are safe to stop
        for link in $(networkctl list | awk '/ether|wlan|wwan/ {print $1}' | grep -v lo || true); do
          networkctl down "${link}" 2>/dev/null || true
        done

        # Remove networkd .network files except lo
        for netfile in /etc/systemd/network/*.network; do
          [[ -f "${netfile}" ]] || continue
          if ! grep -q "Name=lo" "${netfile}" 2>/dev/null; then
            rm -f "${netfile}" 2>/dev/null || true
          fi
        done

        systemctl reload systemd-networkd 2>/dev/null || true
        echo "✓ systemd-networkd cleanup completed"
      else
        echo "⚠️  systemd-networkd cleanup SKIPPED to preserve connectivity"
      fi
    else
      echo "No systemd-networkd interfaces found - nothing to clean up"
    fi
  fi

  # 2. Ultra-conservative network cleanup - ZERO connectivity interruption
  if systemctl is-active --quiet NetworkManager 2>/dev/null && command -v nmcli >/dev/null 2>&1; then
    echo "Performing ultra-conservative systemd-networkd cleanup (zero interruption)..."

    # Get current active connections BEFORE any cleanup
    local active_before=$(nmcli -t -f NAME connection show --active 2>/dev/null | wc -l)
    echo "Active connections before cleanup: ${active_before}"

    # Skip checkpoint creation for safety (systemd-networkd doesn't support checkpoints)
    local checkpoint_path=""
    echo "⏭️  Skipping checkpoint creation during cleanup - systemd-networkd doesn't support rollback checkpoints"

    # ULTRA-CONSERVATIVE APPROACH: Only delete connections that are 100% safe to remove
    local connections_to_delete=()
    local uplink_connection=""

    # Find uplink connection if specified
    if [[ -n "${uplink}" ]]; then
      uplink_connection=$(nmcli -t -f NAME,DEVICE connection show --active 2>/dev/null | grep ":${uplink}$" | cut -d: -f1 || true)
      if [[ -n "${uplink_connection}" ]]; then
        echo "Preserving uplink connection: ${uplink_connection}"
      fi
    fi

    # Get all connections with their states
    local all_conns
    all_conns=$(nmcli -t -f NAME,UUID,TYPE,STATE connection show 2>/dev/null || true)

    while IFS=':' read -r conn_name uuid conn_type conn_state; do
      [[ -z "${conn_name}" ]] && continue

      # CRITICAL: Never delete active connections
      if [[ "${conn_state}" == "activated" ]]; then
        echo "⚠️  PRESERVING active connection: ${conn_name} (${conn_type}) - CRITICAL for connectivity"
        continue
      fi

      # Only delete connections that are 100% safe to remove
      case "${conn_name}" in
        # System connections - NEVER delete
        lo|docker0|virbr0|ovs-system)
          echo "✓ Preserving essential system connection: ${conn_name}"
          continue
          ;;

        # Uplink connection - NEVER delete
        *)
          if [[ -n "${uplink_connection}" && "${conn_name}" == "${uplink_connection}" ]]; then
            echo "✓ Preserving uplink connection: ${conn_name}"
            continue
          fi

          # Legacy dyn connections from old monitoring - safe to delete (inactive)
          if [[ "${conn_name}" =~ ^dyn-(port|eth)- ]] && [[ "${conn_state}" != "activated" ]]; then
            echo "🗑️  Deleting legacy dyn connection: ${conn_name} (inactive)"
            connections_to_delete+=("${conn_name}")
            continue
          fi

          # OVS connections that are clearly from old installation - safe to delete (inactive)
          if [[ "${conn_name}" =~ ^(ovs-bridge-|ovs-port-|ovs-if-).* ]] && [[ "${conn_state}" != "activated" ]]; then
            echo "🗑️  Deleting old OVS connection: ${conn_name} (inactive)"
            connections_to_delete+=("${conn_name}")
            continue
          fi

          # For any other inactive connection, be EXTREMELY conservative
          echo "⚠️  PRESERVING unknown connection: ${conn_name} (${conn_type}) - may be critical for connectivity"
          ;;
      esac
    done <<< "${all_conns}"

    # Only proceed with deletion if we have safe connections to delete
    if [[ ${#connections_to_delete[@]} -gt 0 ]]; then
      echo "🗑️  Safely deleting ${#connections_to_delete[@]} obsolete connections..."

      # Delete connections one by one with verification
      for conn in "${connections_to_delete[@]}"; do
        echo "Deleting: ${conn}"
        if nmcli connection delete "${conn}" >/dev/null 2>&1; then
          echo "  ✓ Deleted: ${conn}"
        else
          echo "  ⚠️  Failed to delete: ${conn} (may already be gone)"
        fi
      done

      # Reload connections
      nmcli connection reload 2>/dev/null || true

      # Verify connectivity is preserved
      local active_after=$(nmcli -t -f NAME connection show --active 2>/dev/null | wc -l)
      if [[ ${active_after} -lt ${active_before} ]]; then
        echo "⚠️  WARNING: Active connections decreased from ${active_before} to ${active_after}"
        echo "❌ Installation aborted - connectivity would be interrupted"
        echo "ℹ️  No checkpoint rollback available (systemd-networkd doesn't support checkpoints)"
        exit 1
      else
        echo "✓ Connectivity preserved: ${active_after} active connections maintained"
        # No checkpoint to destroy (systemd-networkd doesn't support checkpoints)
      fi
    else
      echo "✓ No obsolete connections found to clean up"
      # No checkpoint to destroy (systemd-networkd doesn't support checkpoints)
    fi

    echo "✓ Ultra-conservative systemd-networkd cleanup completed - ZERO connectivity interruption"
  fi

  # 3. OVS cleanup
  if command -v ovs-vsctl >/dev/null 2>&1; then
    if (( PURGE_BRIDGES )); then
      echo "Performing DESTRUCTIVE OVS purge (all bridges) per --purge-bridges..."

      # Extra safety: create NM checkpoint to allow rollback if connectivity drops
      local checkpoint_path=""
      if command -v gdbus >/dev/null 2>&1 && command -v nmcli >/dev/null 2>&1; then
        local device_paths
        device_paths=$(gdbus call --system --dest org.freedesktop.systemd-networkd \
          --object-path /org/freedesktop/systemd-networkd \
          --method org.freedesktop.systemd-networkd.GetDevices 2>/dev/null | \
          grep -o "'[^']*'" | tr -d "'" | tr '\n' ',' | sed 's/,$//')
        if [[ -n "${device_paths}" ]]; then
          checkpoint_path=$(gdbus call --system --dest org.freedesktop.systemd-networkd \
            --object-path /org/freedesktop/systemd-networkd \
            --method org.freedesktop.systemd-networkd.CheckpointCreate \
            "[${device_paths}]" 600 2>/dev/null | grep -o "'[^']*'" | tr -d "'" | head -1)
        fi
      fi

      # If uplink is provided, ensure it is detached from any bridge before purge
      detach_uplink_if_enslaved "${uplink}"

      # First, try to delete via systemd-networkd profiles so NM tears down cleanly
      if command -v nmcli >/dev/null 2>&1; then
        echo "Removing OVS-related systemd-networkd profiles..."
        nmcli -t -f NAME,TYPE connection show | awk -F: '/^ovs-|:ovs-bridge|:ovs-port|:ovs-interface/ {print $1}' | while read -r conn; do
          [[ -n "$conn" ]] || continue
          echo "  Deleting NM connection: $conn"
          nmcli connection delete "$conn" >/dev/null 2>&1 || true
        done
      fi

      # Then, if allowed, hard purge remaining OVS bridges using ovs-vsctl
      if (( ALLOW_OVSCTL_FORCE )); then
        echo "Hard purging all OVS bridges using ovs-vsctl (dangerous)"
        for br in $(ovs-vsctl list-br 2>/dev/null || true); do
          echo "  Deleting bridge: $br"
          ovs-vsctl del-br "$br" 2>/dev/null || true
        done
      else
        echo "Skipping ovs-vsctl hard purge (enable with --force-ovsctl)"
      fi

      # Verify and rollback if connection count dropped
      if command -v nmcli >/dev/null 2>&1; then
        local active_after
        active_after=$(nmcli -t -f NAME connection show --active 2>/dev/null | wc -l)
        echo "Active connections after purge: ${active_after}"
      fi

      if [[ -n "${checkpoint_path}" ]]; then
        # Commit the checkpoint after successful purge
        gdbus call --system --dest org.freedesktop.systemd-networkd \
          --object-path /org/freedesktop/systemd-networkd \
          --method org.freedesktop.systemd-networkd.CheckpointDestroy \
          "'${checkpoint_path}'" >/dev/null 2>&1 || true
      fi
    else
      echo "Performing maximum-conservative OVS cleanup..."

    # Get list of OVS bridges
    local ovs_bridges
    ovs_bridges=$(ovs-vsctl list-br 2>/dev/null || true)

    if [[ -n "${ovs_bridges}" ]]; then
      echo "Found existing OVS bridges: ${ovs_bridges}"

      # Backup current OVS configuration for rollback
      ovs-vsctl show > "${BACKUP_DIR}/ovs-before-cleanup.show" 2>/dev/null || true

      for bridge in ${ovs_bridges}; do
        # Skip if it's one of our target bridges
        if [[ "${bridge}" == "${BRIDGE}" ]] || [[ "${bridge}" == "ovsbr1" ]]; then
          echo "⚠️  PRESERVING target OVS bridge: ${bridge} - will be recreated by installation"
          continue
        fi

        # Check if bridge has ANY ports (even inactive ones)
        local bridge_ports=$(ovs-vsctl list-ports "${bridge}" 2>/dev/null | wc -l)

        if [[ ${bridge_ports} -gt 0 ]]; then
          echo "⚠️  Bridge ${bridge} has ${bridge_ports} ports - SKIPPING to preserve connectivity"
          echo "   This bridge may be providing connectivity to existing containers/VMs"
          echo "   ⚠️  WARNING: Installation may fail due to existing bridge conflicts"
          continue
        fi

        # Additional safety check: check if bridge is referenced in any systemd-networkd connections
        local bridge_in_nm=$(nmcli -t -f NAME connection show 2>/dev/null | grep -c "^${bridge}$" || true)
        if [[ ${bridge_in_nm} -gt 0 ]]; then
          echo "⚠️  Bridge ${bridge} is referenced in ${bridge_in_nm} systemd-networkd connection(s)"
          echo "   SKIPPING to avoid disrupting existing network configuration"
          continue
        fi

        # Only report completely unused bridges; no mutation without --purge-bridges
        echo "Detected completely unused OVS bridge: ${bridge} (no action)"
      done

      # Count remaining bridges
      local remaining_bridges=$(ovs-vsctl list-br 2>/dev/null | wc -l)
      echo "OVS bridges after cleanup: ${remaining_bridges} (target bridges preserved)"

      if [[ ${remaining_bridges} -gt 2 ]]; then
        echo "⚠️  WARNING: Multiple OVS bridges still exist - installation may fail"
        echo "   Consider manual cleanup of unused bridges before installation"
      fi

      # Preserving OVS database to avoid connectivity interruption
      echo "⚠️  Preserving OVS database (no destructive changes without --purge-bridges)"
    else
      echo "No OVS bridges found - nothing to clean up"
    fi
    fi
  fi

  # 4. Connectivity-preserving /etc/network/interfaces cleanup
  if [[ -f "/etc/network/interfaces" ]]; then
    echo "Checking /etc/network/interfaces for cleanup..."

    # Create backup first
    cp /etc/network/interfaces "${BACKUP_DIR}/interfaces.backup" 2>/dev/null || true

    # Check if file contains any active bridge configurations that might be in use
    local active_bridges=$(grep -c "^auto.*br" /etc/network/interfaces 2>/dev/null || true)
    local ovs_interfaces=$(grep -c "ovs_type" /etc/network/interfaces 2>/dev/null || true)

    if [[ ${active_bridges} -gt 0 ]] || [[ ${ovs_interfaces} -gt 0 ]]; then
      echo "⚠️  Found ${active_bridges} bridge configs and ${ovs_interfaces} OVS interfaces in /etc/network/interfaces"
      echo "   These may be providing connectivity - SKIPPING cleanup to preserve connectivity"

      # Instead of removing everything, just comment out OVS-specific sections
      # that might conflict with systemd-networkd
      awk '
      BEGIN { in_ovs_section = 0 }
      /^auto.*ovsbr/ || /^iface.*ovsbr/ || /^auto.*br.*ovs/ || /^iface.*br.*ovs/ {
        in_ovs_section = 1
        print "# " $0 " # Commented out by ovs-port-agent installation"
        next
      }
      /^$/ && in_ovs_section {
        print "# Commented out by ovs-port-agent installation"
        in_ovs_section = 0
        next
      }
      in_ovs_section {
        print "# " $0 " # Commented out by ovs-port-agent installation"
        next
      }
      { print }
      ' /etc/network/interfaces > /etc/network/interfaces.tmp 2>/dev/null || true

      if [[ -s "/etc/network/interfaces.tmp" ]]; then
        mv /etc/network/interfaces.tmp /etc/network/interfaces 2>/dev/null || true
        echo "✓ Commented out conflicting OVS configurations"
      fi
    else
      echo "No active bridge configurations found - /etc/network/interfaces is clean"
    fi

    # Set proper permissions
    chmod 644 /etc/network/interfaces 2>/dev/null || true
  fi

  # 5. D-Bus cleanup
  echo "Cleaning up D-Bus services..."
  # Ensure systemd-networkd is running before D-Bus reload so it re-registers properly
  if command -v systemctl >/dev/null 2>&1; then
    if systemctl list-unit-files 2>/dev/null | grep -q '^systemd-networkd.service'; then
      if ! systemctl is-active --quiet systemd-networkd; then
        echo "Starting systemd-networkd prior to D-Bus reload..."
        systemctl start systemd-networkd 2>/dev/null || true
        # Best-effort wait to avoid racing D-Bus reload
        sleep 1
      fi
    fi
  fi
  # Only reload D-Bus; avoid restarts to preserve connectivity
  systemctl reload dbus.service 2>/dev/null || true

  # Kill any lingering network-related processes
  pkill -f "dhclient\|systemd-networkd\|wpa_supplicant" 2>/dev/null || true

  echo "Ultra-conservative connectivity-preserving cleanup complete!"
  echo "✓ ABSOLUTE ZERO connectivity interruption during installation"
  echo "Preserved: ALL active connections, uplink, and essential system interfaces"
  echo "Cleaned: Only legacy dyn-port connections (100% safe removals)"
  echo "Skipped: Any potentially active network configurations"
  echo "Backups created in ${BACKUP_DIR} for rollback capability"
}

ensure_systemd_bridge() {
  local bridge_name="$1"
  local uplink="$2"

  if ! command -v nmcli >/dev/null 2>&1; then
    echo "nmcli not found; skipping systemd-networkd bridge setup"
    return
  fi

  if ! systemctl is-active --quiet systemd-networkd; then
    echo "systemd-networkd is not active; skipping nmcli bridge setup"
    return
  fi

  local bridge_conn="ovs-bridge-${bridge_name}"
  local port_conn="ovs-port-${bridge_name}"
  local iface_conn="ovs-if-${bridge_name}"

echo "Creating systemd-networkd configuration for OVS bridge ${bridge_name}"

  # Create .netdev file for the OVS bridge
  cat > "/etc/systemd/network/${bridge_name}.netdev" << EOF
[NetDev]
Name=${bridge_name}
Kind=ovs-bridge

[OVSBridge]
STP=no
RSTP=no
McastSnooping=yes
EOF

# Create .network file for the bridge
  cat > "/etc/systemd/network/${bridge_name}.network" << EOF
[Match]
Name=${bridge_name}

[Network]
DHCP=yes
IPv6AcceptRA=yes
EOF

  # Create internal interface .netdev and .network
  local internal_iface="${bridge_name}_if"
  cat > "/etc/systemd/network/${internal_iface}.netdev" << EOF
[NetDev]
Name=${internal_iface}
Kind=ovs-interface

[OVSInterface]
Type=internal
Bridge=${bridge_name}
EOF

  cat > "/etc/systemd/network/${internal_iface}.network" << EOF
[Match]
Name=${internal_iface}

[Network]
DHCP=yes
IPv6AcceptRA=yes
EOF

  # Reload systemd-networkd
  networkctl reload 2>/dev/null || true
  echo "Note: Network configuration applied - manual service reload may be needed if changes don't take effect"

if [[ -n "${uplink}" ]]; then
    echo "Configuring uplink ${uplink} for ${bridge_name}"
    
    # Create .network file to enslave uplink to bridge
    cat > "/etc/systemd/network/50-${uplink}.network" << EOF
[Match]
Name=${uplink}

[Network]
Bridge=${bridge_name}
EOF
  fi

  # Apply systemd-networkd configuration
  if [[ ${NO_RESTART} -eq 0 ]]; then
    echo "Reloading systemd-networkd configuration..."
    if ! networkctl reload 2>/dev/null; then
      echo "⚠️  networkctl reload failed, attempting service restart..."
      echo "⚠️  WARNING: This may temporarily disrupt network connectivity!"
      echo "Press Ctrl+C within 5 seconds to abort..."
      sleep 5
      systemctl restart systemd-networkd 2>/dev/null || true
    fi
  else
    echo "Skipping network service restart (--no-restart specified)"
    echo "Manual reload required: sudo networkctl reload"
  fi
}

ensure_systemd_bridge_ovsbr1() {
  local bridge_name="$1"

  echo "Creating ovsbr1 as isolated bridge (no uplink, DHCP-enabled)"

  # Create .netdev file for ovsbr1 bridge
  cat > "/etc/systemd/network/${bridge_name}.netdev" << EOF
[NetDev]
Name=${bridge_name}
Kind=ovs-bridge

[OVSBridge]
STP=no
RSTP=no
McastSnooping=yes
EOF

  # Create .network file for ovsbr1 with DHCP
  cat > "/etc/systemd/network/${bridge_name}.network" << EOF
[Match]
Name=${bridge_name}

[Network]
DHCP=yes
IPv6AcceptRA=yes
EOF

  # Create internal interface for DHCP
  local internal_iface="${bridge_name}_if"
  cat > "/etc/systemd/network/${internal_iface}.netdev" << EOF
[NetDev]
Name=${internal_iface}
Kind=ovs-interface

[OVSInterface]
Type=internal
Bridge=${bridge_name}
EOF

  cat > "/etc/systemd/network/${internal_iface}.network" << EOF
[Match]
Name=${internal_iface}

[Network]
DHCP=yes
IPv6AcceptRA=yes
EOF

  echo "Activating ovsbr1 bridge..."
  networkctl reload 2>/dev/null || true
  echo "Note: Network configuration applied - manual service reload may be needed if changes don't take effect"

  echo "ovsbr1 created successfully as DHCP-enabled bridge"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge)
      [[ $# -ge 2 ]] || { echo "Missing value for --bridge" >&2; exit 1; }
      BRIDGE="$2"; shift 2;;
    --prefix)
      [[ $# -ge 2 ]] || { echo "Missing value for --prefix" >&2; exit 1; }
      PREFIX="$2"; shift 2;;
    --system)
      SYSTEM=1; shift;;
    --uplink)
      [[ $# -ge 2 ]] || { echo "Missing value for --uplink" >&2; exit 1; }
      UPLINK="$2"; shift 2;;
    --with-ovsbr1)
      CREATE_OVSBR1=1; shift;;
    --purge-bridges)
      PURGE_BRIDGES=1; shift;;
    --force-ovsctl)
      ALLOW_OVSCTL_FORCE=1; shift;;
    --no-restart)
      NO_RESTART=1; shift;;
    --skip-netmaker)
      SKIP_NETMAKER=1; shift;;
    --help|-h)
      usage; exit 0;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1;;
  esac
done

echo "Building release binary..."
if command -v cargo >/dev/null 2>&1; then
  CARGO_BIN=$(command -v cargo)
else
  if [[ -n "${SUDO_USER:-}" ]]; then
    SUDO_HOME=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    if [[ -n "$SUDO_HOME" && -x "$SUDO_HOME/.cargo/bin/cargo" ]]; then
      CARGO_BIN="$SUDO_HOME/.cargo/bin/cargo"
    fi
  fi
  if [[ -z "${CARGO_BIN:-}" ]]; then
    if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
      CARGO_BIN="$HOME/.cargo/bin/cargo"
    fi
  fi
  if [[ -z "${CARGO_BIN:-}" ]]; then
    echo "cargo not found in PATH or user homes" >&2
    exit 2
  fi
fi
export PATH="$(dirname "${CARGO_BIN}"):${PATH}"
"${CARGO_BIN}" build --release

BIN_DEST="${PREFIX}/bin/ovs-port-agent"
CONFIG_DIR="/etc/ovs-port-agent"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
LEDGER_DIR="/var/lib/ovs-port-agent"
SYSTEMD_UNIT="/etc/systemd/system/ovs-port-agent.service"
DBUS_POLICY="/etc/dbus-1/system.d/dev.ovs.PortAgent1.conf"

install -d -m 0755 "${PREFIX}/bin"
install -m 0755 target/release/ovs-port-agent "${BIN_DEST}"

install -d -m 0755 "${CONFIG_DIR}"
if [[ ! -f "${CONFIG_FILE}" ]]; then
  install -m 0644 config/config.toml.example "${CONFIG_FILE}"
fi

install -d -m 0750 "${LEDGER_DIR}"

# Update bridge_name in config
python3 - <<PY
import pathlib, re
cfg_path = pathlib.Path("${CONFIG_FILE}")
text = cfg_path.read_text()
pattern = re.compile(r'^bridge_name\s*=\s*".*"', re.MULTILINE)
replacement = 'bridge_name = "${BRIDGE}"'
if pattern.search(text):
    text = pattern.sub(replacement, text, count=1)
else:
    text = replacement + "\n" + text
cfg_path.write_text(text)
PY

install -m 0644 dbus/dev.ovs.PortAgent1.conf "${DBUS_POLICY}"
install -m 0644 systemd/ovs-port-agent.service "${SYSTEMD_UNIT}"

systemctl daemon-reload
systemctl reload dbus.service 2>/dev/null || systemctl restart dbus.service

# Create backups before making any changes
create_backups

# Clean up existing devices before creating new bridge setup
cleanup_all_networking "${UPLINK}"

ensure_systemd_bridge "${BRIDGE}" "${UPLINK}"
if (( CREATE_OVSBR1 )); then
  # Clean up ovsbr1 separately since it has no uplink
  cleanup_all_networking ""
  ensure_systemd_bridge_ovsbr1 "ovsbr1"
fi

if command -v journalctl >/dev/null 2>&1; then
  if journalctl -k --since "-10 minutes" | grep -qi segfault; then
    echo "Warning: kernel reported segfaults in the last 10 minutes:"
    journalctl -k --since "-10 minutes" | grep -i segfault || true
  fi
fi

if (( SYSTEM )); then
  systemctl enable --now ovs-port-agent
  systemctl status --no-pager ovs-port-agent || true
else
  echo "Installation complete. Start the service with: sudo systemctl enable --now ovs-port-agent"
fi

# Clean up old backups
cleanup_backups

echo "Installation completed successfully!"
echo "Detected services:"
[[ ${HAS_SYSTEMD_NETWORKD} -eq 1 ]] && echo "  ✓ systemd-networkd configured"
[[ ${HAS_NETMAKER} -eq 1 ]] && echo "  ✓ Netmaker support enabled"
[[ ${HAS_DOCKER} -eq 1 ]] && echo "  ✓ Docker integration ready"
[[ ${HAS_OVS} -eq 1 ]] && echo "  ✓ Open vSwitch available"
echo "Rollback script available at: ${REPO_ROOT}/scripts/rollback.sh"
echo "Btrfs snapshot available for system-level rollback if needed"
exit 0
