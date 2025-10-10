#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/.." && pwd)
cd "${REPO_ROOT}"

usage() {
  cat <<USAGE
Usage: ./scripts/install.sh [options]

Options:
  --bridge NAME     Set bridge_name in the agent config (default: ovsbr0)
  --prefix DIR      Installation prefix for the binary (default: /usr/local)
  --uplink IFACE    Physical interface to enslave to the bridge (optional)
  --with-ovsbr1     Also create secondary bridge ovsbr1 (DHCP)
  --ovsbr1-uplink IFACE  Physical interface to attach as ovsbr1 uplink (optional)
  --system          Enable and start the systemd service after installing
  --help            Show this help message

Note: Before installation, all NetworkManager connections except uplink and lo
will be removed to ensure a clean slate for the OVS bridge setup.
USAGE
}

BRIDGE="ovsbr0"
PREFIX="/usr/local"
SYSTEM=0
UPLINK=""
CREATE_OVSBR1=0
OVSBR1_UPLINK=""

cleanup_devices() {
  local uplink="$1"

  if ! command -v nmcli >/dev/null 2>&1; then
    echo "nmcli not found; skipping NetworkManager cleanup"
    return
  fi

  if ! systemctl is-active --quiet NetworkManager; then
    echo "NetworkManager is not active; skipping cleanup"
    return
  fi

  echo "Cleaning up existing NetworkManager connections..."

  # Get list of all connections except system ones
  local all_conns
  all_conns=$(nmcli -t -f NAME,UUID,TYPE connection show || true)

  local connections_to_delete=()
  local uplink_connection=""

  # Find uplink connection if specified
  if [[ -n "${uplink}" ]]; then
    # Look for active connection on the uplink interface
    uplink_connection=$(nmcli -t -f NAME,DEVICE connection show --active | grep ":${uplink}$" | cut -d: -f1 || true)

    # If not found in active connections, look in all connections
    if [[ -z "${uplink_connection}" ]]; then
      uplink_connection=$(nmcli -t -f NAME,DEVICE connection show | grep ":${uplink}$" | cut -d: -f1 || true)
    fi

    if [[ -n "${uplink_connection}" ]]; then
      echo "Preserving uplink connection: ${uplink_connection} (interface: ${uplink})"
    else
      echo "Warning: Uplink interface ${uplink} not found in NetworkManager connections"
    fi
  fi

  # Parse connections and decide what to delete
  while IFS=':' read -r conn_name uuid conn_type; do
    # Preserve essential system connections
    case "${conn_name}" in
      lo|docker0|virbr0|ovs-system)
        echo "Preserving system connection: ${conn_name}"
        continue
        ;;
      *)
        # Check if this is an uplink connection
        if [[ -n "${uplink_connection}" && "${conn_name}" == "${uplink_connection}" ]]; then
          echo "Preserving uplink connection: ${conn_name}"
          continue
        fi
        ;;
    esac

    # Delete everything else (including old OVS bridges, ports, interfaces)
    connections_to_delete+=("${conn_name}")
  done <<< "${all_conns}"

  # Delete the connections we identified
  for conn in "${connections_to_delete[@]}"; do
    echo "Deleting connection: ${conn}"
    nmcli connection delete "${conn}" >/dev/null 2>&1 || true
  done

  echo "Cleanup complete. Preserved: uplink (${uplink:-none}), lo, and system connections"

  # Also clean up OVS bridges if ovs-vsctl is available
  if command -v ovs-vsctl >/dev/null 2>&1; then
    echo "Checking for existing OVS bridges to clean up..."

    # Get list of OVS bridges
    local ovs_bridges
    ovs_bridges=$(ovs-vsctl list-br 2>/dev/null || true)

    for bridge in ${ovs_bridges}; do
      # Skip if it's one of our target bridges or doesn't exist
      if [[ "${bridge}" == "${BRIDGE}" ]] || [[ "${bridge}" == "ovsbr1" ]]; then
        echo "Preserving OVS bridge: ${bridge}"
        continue
      fi

      echo "Removing OVS bridge: ${bridge}"
      ovs-vsctl del-br "${bridge}" 2>/dev/null || true
    done
  fi
}

ensure_nm_bridge() {
  local bridge_name="$1"
  local uplink="$2"

  if ! command -v nmcli >/dev/null 2>&1; then
    echo "nmcli not found; skipping NetworkManager bridge setup"
    return
  fi

  if ! systemctl is-active --quiet NetworkManager; then
    echo "NetworkManager is not active; skipping nmcli bridge setup"
    return
  fi

  local bridge_conn="ovs-bridge-${bridge_name}"
  local port_conn="ovs-port-${bridge_name}"
  local iface_conn="ovs-if-${bridge_name}"

  nmcli connection delete "${bridge_conn}" >/dev/null 2>&1 || true
  nmcli connection delete "${port_conn}" >/dev/null 2>&1 || true
  nmcli connection delete "${iface_conn}" >/dev/null 2>&1 || true

  echo "Provisioning NetworkManager bridge profiles for ${bridge_name}"
  nmcli connection add type ovs-bridge \
    conn.interface "${bridge_name}" \
    con-name "${bridge_conn}" >/dev/null

  nmcli connection add type ovs-port \
    conn.interface "${bridge_name}" \
    master "${bridge_name}" \
    con-name "${port_conn}" >/dev/null

  nmcli connection add type ovs-interface \
    slave-type ovs-port \
    conn.interface "${bridge_name}" \
    master "${port_conn}" \
    con-name "${iface_conn}" \
    ipv4.method auto \
    ipv6.method disabled >/dev/null

  if [[ -n "${uplink}" ]]; then
    local uplink_port_conn="ovs-port-${bridge_name}-${uplink}"
    local uplink_iface_conn="ovs-if-${bridge_name}-${uplink}"

    nmcli connection delete "${uplink_port_conn}" >/dev/null 2>&1 || true
    nmcli connection delete "${uplink_iface_conn}" >/dev/null 2>&1 || true

    echo "Provisioning uplink ${uplink} for ${bridge_name}"
    nmcli connection add type ovs-port \
      conn.interface "${uplink}" \
      master "${bridge_name}" \
      con-name "${uplink_port_conn}" >/dev/null

    nmcli connection add type ethernet \
      conn.interface "${uplink}" \
      master "${uplink_port_conn}" \
      con-name "${uplink_iface_conn}" >/dev/null

    nmcli connection up "${uplink_iface_conn}" >/dev/null 2>&1 || true
  fi

  nmcli connection up "${bridge_conn}" >/dev/null 2>&1 || true
  nmcli connection up "${iface_conn}"  >/dev/null 2>&1 || true
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
    --ovsbr1-uplink)
      [[ $# -ge 2 ]] || { echo "Missing value for --ovsbr1-uplink" >&2; exit 1; }
      OVSBR1_UPLINK="$2"; shift 2;;
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

if [[ ${EUID} -ne 0 ]]; then
  echo "This installer must be run as root (try: sudo ./scripts/install.sh ...)" >&2
  exit 1
fi

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

# Clean up existing devices before creating new bridge setup
cleanup_devices "${UPLINK}"

ensure_nm_bridge "${BRIDGE}" "${UPLINK}"
if (( CREATE_OVSBR1 )); then
  ensure_nm_bridge "ovsbr1" "${OVSBR1_UPLINK}"
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
