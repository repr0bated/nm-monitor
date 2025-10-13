#!/usr/bin/env bash
# Simple installation script using the network plugin for declarative setup
# This is a clean, modern approach compared to the legacy install.sh

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=========================================="
echo " ovs-port-agent Installation"
echo " Using Network Plugin (Declarative)"
echo "=========================================="
echo ""

# Root check
if [[ ${EUID} -ne 0 ]]; then
  echo -e "${RED}ERROR: Must run as root${NC}" >&2
  echo "Usage: sudo $0 [--network-config FILE] [--system]"
  exit 1
fi

# Defaults
NETWORK_CONFIG=""
ENABLE_SERVICE=0
PREFIX="/usr/local"
WITH_OVSBR1=0
INTROSPECT=0

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --network-config)
      [[ $# -ge 2 ]] || { echo "Missing value for --network-config" >&2; exit 1; }
      NETWORK_CONFIG="$2"
      shift 2
      ;;
    --introspect)
      INTROSPECT=1
      shift
      ;;
    --with-ovsbr1)
      WITH_OVSBR1=1
      shift
      ;;
    --system)
      ENABLE_SERVICE=1
      shift
      ;;
    --prefix)
      [[ $# -ge 2 ]] || { echo "Missing value for --prefix" >&2; exit 1; }
      PREFIX="$2"
      shift 2
      ;;
    --help|-h)
      cat <<HELP
Usage: $0 [options]

Simple installation using declarative network configuration.

Options:
  --network-config FILE   Path to network config YAML (required*)
  --introspect            Auto-detect network and generate config (replaces --network-config)
  --with-ovsbr1           Add ovsbr1 isolated bridge (for Docker/containers)
  --system                Enable and start systemd service after install
  --prefix DIR            Installation prefix (default: /usr/local)
  --help                  Show this help

*Not required if using --introspect

Examples:
  # Use existing config
  sudo $0 --network-config config/examples/network-ovs-bridges.yaml --system
  
  # Auto-detect network (introspect)
  sudo $0 --introspect --system
  
  # Introspect + add ovsbr1
  sudo $0 --introspect --with-ovsbr1 --system

This installer:
  1. Builds the release binary
  2. Installs binary, config, and systemd files
  3. Applies your declarative network configuration
  4. Optionally enables the systemd service

Network config examples in config/examples/:
  - test-ovs-simple.yaml      (safe test: isolated bridge)
  - network-ovs-bridges.yaml  (production: ovsbr0 + ovsbr1)
  - network-static-ip.yaml    (VPS: static IP + uplink)

HELP
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Run with --help for usage"
      exit 1
      ;;
  esac
done

# Validate config mode (but don't generate yet if introspecting)
if [[ ${INTROSPECT} -eq 0 ]]; then
  # Validate network config
  if [[ -z "${NETWORK_CONFIG}" ]]; then
    echo -e "${RED}ERROR: --network-config is required (or use --introspect)${NC}" >&2
    echo ""
    echo "Options:"
    echo "  1. Use existing config: --network-config FILE"
    echo "  2. Auto-detect: --introspect"
    echo ""
    echo "Example configs available:"
    echo "  config/examples/test-ovs-simple.yaml"
    echo "  config/examples/network-ovs-bridges.yaml"
    echo "  config/examples/network-static-ip.yaml"
    echo ""
    echo "Run with --help for more info"
    exit 1
  fi

  if [[ ! -f "${NETWORK_CONFIG}" ]]; then
    echo -e "${RED}ERROR: Network config not found: ${NETWORK_CONFIG}${NC}" >&2
    exit 1
  fi
fi

echo "Configuration:"
if [[ ${INTROSPECT} -eq 1 ]]; then
  echo "  Network Config: Auto-detect (introspect)"
else
  echo "  Network Config: ${NETWORK_CONFIG}"
fi
echo "  Install Prefix: ${PREFIX}"
echo "  Add ovsbr1: $([ ${WITH_OVSBR1} -eq 1 ] && echo 'Yes' || echo 'No')"
echo "  Enable Service: $([ ${ENABLE_SERVICE} -eq 1 ] && echo 'Yes' || echo 'No')"
echo ""

# Check prerequisites
echo "Checking prerequisites..."

# Find cargo (handle sudo case where cargo is in user's home)
CARGO_BIN=""
if command -v cargo >/dev/null 2>&1; then
  CARGO_BIN=$(command -v cargo)
else
  # When running under sudo, check the real user's cargo
  if [[ -n "${SUDO_USER:-}" ]]; then
    SUDO_HOME=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    if [[ -n "$SUDO_HOME" && -x "$SUDO_HOME/.cargo/bin/cargo" ]]; then
      CARGO_BIN="$SUDO_HOME/.cargo/bin/cargo"
      echo "Found cargo in $SUDO_USER's home: $CARGO_BIN"
    fi
  fi
  
  # Last resort: check current user's home
  if [[ -z "${CARGO_BIN}" && -x "$HOME/.cargo/bin/cargo" ]]; then
    CARGO_BIN="$HOME/.cargo/bin/cargo"
  fi
fi

if [[ -z "${CARGO_BIN}" ]]; then
  echo -e "${RED}ERROR: cargo not found${NC}" >&2
  echo "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  echo ""
  echo "If you just installed Rust, run: source \$HOME/.cargo/env"
  exit 1
fi

echo "Using cargo: ${CARGO_BIN}"

if ! command -v ovs-vsctl >/dev/null 2>&1; then
  echo -e "${YELLOW}WARNING: openvswitch-switch not installed${NC}"
  echo "Install: apt-get install openvswitch-switch"
  read -p "Continue anyway? [y/N] " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

if ! systemctl is-active --quiet openvswitch-switch 2>/dev/null; then
  echo -e "${YELLOW}WARNING: openvswitch-switch not running${NC}"
  echo "Starting openvswitch-switch..."
  systemctl start openvswitch-switch || {
    echo -e "${RED}ERROR: Failed to start openvswitch-switch${NC}" >&2
    exit 1
  }
fi

echo -e "${GREEN}✓${NC} Prerequisites OK"
echo ""

# Build
echo "=========================================="
echo " Step 1: Building Release Binary"
echo "=========================================="
echo ""

cd "${REPO_ROOT}"

# Ensure cargo is in PATH
export PATH="$(dirname "${CARGO_BIN}"):${PATH}"

"${CARGO_BIN}" build --release

echo ""
echo -e "${GREEN}✓${NC} Build complete"
echo ""

# Install files
echo "=========================================="
echo " Step 2: Installing Files"
echo "=========================================="
echo ""

BIN_DEST="${PREFIX}/bin/ovs-port-agent"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="/etc/ovs-port-agent"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
LEDGER_DIR="/var/lib/ovs-port-agent"
SYSTEMD_UNIT="/etc/systemd/system/ovs-port-agent.service"
DBUS_POLICY="/etc/dbus-1/system.d/dev.ovs.PortAgent1.conf"

# Install binary
echo "Installing binary to ${BIN_DEST}..."
install -d -m 0755 "${PREFIX}/bin"
install -m 0755 target/release/ovs-port-agent "${BIN_DEST}"

# Install config
echo "Installing config to ${CONFIG_DIR}..."
install -d -m 0755 "${CONFIG_DIR}"
if [[ ! -f "${CONFIG_FILE}" ]]; then
  install -m 0644 config/config.toml.example "${CONFIG_FILE}"
else
  echo "  (config already exists, not overwriting)"
fi

# Create ledger directory
echo "Creating ledger directory ${LEDGER_DIR}..."
install -d -m 0750 "${LEDGER_DIR}"

# Install D-Bus policy
echo "Installing D-Bus policy..."
install -m 0644 dbus/dev.ovs.PortAgent1.conf "${DBUS_POLICY}"

# Install systemd unit
echo "Installing systemd service..."
install -m 0644 systemd/ovs-port-agent.service "${SYSTEMD_UNIT}"

# Reload systemd
systemctl daemon-reload
systemctl reload dbus.service 2>/dev/null || systemctl restart dbus.service

echo ""
echo -e "${GREEN}✓${NC} Files installed"
echo ""

# NOW run introspection if needed (binary is installed)
if [[ ${INTROSPECT} -eq 1 ]]; then
  echo "=========================================="
  echo " Step 3: Auto-Detecting Network"
  echo "=========================================="
  echo ""
  
  INTROSPECT_SCRIPT="${SCRIPT_DIR}/introspect-network.sh"
  
  if [[ ! -f "${INTROSPECT_SCRIPT}" ]]; then
    echo -e "${RED}ERROR: Introspection script not found${NC}" >&2
    echo "Expected: ${INTROSPECT_SCRIPT}"
    exit 1
  fi
  
  # Generate config to temp file
  NETWORK_CONFIG="/tmp/network-introspected-$$.yaml"
  "${INTROSPECT_SCRIPT}" "${NETWORK_CONFIG}" || {
    echo -e "${RED}ERROR: Network introspection failed${NC}" >&2
    exit 1
  }
  
  echo ""
  echo "Using auto-detected configuration"
  echo ""
fi

# Apply network configuration
echo "=========================================="
echo " Step 4: Apply Network Configuration"
echo "=========================================="
echo ""

echo "Config: ${NETWORK_CONFIG}"
echo ""

# Add ovsbr1 to config if requested
FINAL_CONFIG="${NETWORK_CONFIG}"
if [[ ${WITH_OVSBR1} -eq 1 ]]; then
  echo "Adding ovsbr1 isolated bridge to configuration..."
  FINAL_CONFIG="/tmp/network-config-with-ovsbr1-$$.yaml"
  
  # Read original config and add ovsbr1
  python3 - <<'PYTHON' "${NETWORK_CONFIG}" "${FINAL_CONFIG}"
import sys
import yaml

with open(sys.argv[1], 'r') as f:
    config = yaml.safe_load(f)

# Check if ovsbr1 already exists
existing_names = [iface['name'] for iface in config.get('network', {}).get('interfaces', [])]
if 'ovsbr1' not in existing_names:
    # Add ovsbr1 configuration
    ovsbr1_config = {
        'name': 'ovsbr1',
        'type': 'ovs-bridge',
        'ipv4': {
            'enabled': True,
            'dhcp': False,
            'address': [
                {'ip': '172.18.0.1', 'prefix': 16}
            ]
        }
    }
    config['network']['interfaces'].append(ovsbr1_config)
    print("  ✓ Added ovsbr1 (172.18.0.1/16) to configuration")
else:
    print("  ℹ️  ovsbr1 already in config, using existing configuration")

with open(sys.argv[2], 'w') as f:
    yaml.dump(config, f, default_flow_style=False, sort_keys=False)
PYTHON
  
  echo "  Using temporary config: ${FINAL_CONFIG}"
  echo ""
fi

# Show diff first
echo "Calculating changes (dry run)..."
echo "---"
"${BIN_DEST}" show-diff "${FINAL_CONFIG}" || {
  echo -e "${RED}ERROR: Failed to calculate diff${NC}" >&2
  echo "Check your network config syntax"
  exit 1
}
echo "---"
echo ""

read -p "Apply these changes? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Installation cancelled"
  [[ -f "${FINAL_CONFIG}" && "${FINAL_CONFIG}" != "${NETWORK_CONFIG}" ]] && rm -f "${FINAL_CONFIG}"
  echo "To apply later: sudo ovs-port-agent apply-state ${NETWORK_CONFIG}"
  exit 0
fi

# ATOMIC HANDOVER: Create pre-installation backup
echo "=========================================="
echo "Creating pre-installation backup..."
BACKUP_DIR="/var/lib/ovs-port-agent/backups"
mkdir -p "${BACKUP_DIR}"
BACKUP_TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Backup current network state
if command -v networkctl >/dev/null 2>&1; then
  networkctl list > "${BACKUP_DIR}/pre-install-networkctl-${BACKUP_TIMESTAMP}.txt" 2>/dev/null || true
  ip addr show > "${BACKUP_DIR}/pre-install-ip-addr-${BACKUP_TIMESTAMP}.txt" 2>/dev/null || true
fi

# Backup OVS configuration
if command -v ovs-vsctl >/dev/null 2>&1; then
  ovs-vsctl show > "${BACKUP_DIR}/pre-install-ovs-${BACKUP_TIMESTAMP}.txt" 2>/dev/null || true
fi

# Get active interface count BEFORE
ACTIVE_BEFORE=$(ip link show up | grep -c "^[0-9]" || echo "0")
echo "Active interfaces before: ${ACTIVE_BEFORE}"
echo "Backup saved to: ${BACKUP_DIR}"
echo ""

# Apply configuration
echo "Applying network configuration with atomic handover..."
"${BIN_DEST}" apply-state "${FINAL_CONFIG}" || {
  [[ -f "${FINAL_CONFIG}" && "${FINAL_CONFIG}" != "${NETWORK_CONFIG}" ]] && rm -f "${FINAL_CONFIG}"
  echo -e "${RED}ERROR: Failed to apply network config${NC}" >&2
  echo ""
  echo "Troubleshooting:"
  echo "  1. Check OVS is running: sudo systemctl status openvswitch-switch"
  echo "  2. Check config syntax: sudo ovs-port-agent show-diff ${NETWORK_CONFIG}"
  echo "  3. Check logs: sudo journalctl -xe"
  echo "  4. Restore from backup: ${BACKUP_DIR}"
  exit 1
}

# ATOMIC HANDOVER: Verify connectivity preserved
sleep 2  # Allow network to settle
ACTIVE_AFTER=$(ip link show up | grep -c "^[0-9]" || echo "0")
echo "Active interfaces after: ${ACTIVE_AFTER}"

if [[ ${ACTIVE_AFTER} -lt ${ACTIVE_BEFORE} ]]; then
  echo -e "${YELLOW}WARNING: Active interface count decreased${NC}"
  echo "This might indicate connectivity issues"
  echo "Backup available at: ${BACKUP_DIR}"
else
  echo -e "${GREEN}✓${NC} Connectivity preserved (${ACTIVE_AFTER} interfaces active)"
fi

echo ""
echo -e "${GREEN}✓${NC} Network configuration applied"

# Cleanup temp config
[[ -f "${FINAL_CONFIG}" && "${FINAL_CONFIG}" != "${NETWORK_CONFIG}" ]] && rm -f "${FINAL_CONFIG}"

echo ""

# Verify
echo "Verifying network state..."
echo "---"
"${BIN_DEST}" query-state --plugin net | head -40 || true
echo "---"
echo ""

# Enable service
if [[ ${ENABLE_SERVICE} -eq 1 ]]; then
  echo "=========================================="
  echo " Step 5: Enable Service"
  echo "=========================================="
  echo ""
  
  systemctl enable ovs-port-agent
  systemctl start ovs-port-agent
  
  sleep 2
  
  if systemctl is-active --quiet ovs-port-agent; then
    echo -e "${GREEN}✓${NC} Service is running"
    echo ""
    systemctl status --no-pager ovs-port-agent || true
  else
    echo -e "${YELLOW}WARNING: Service failed to start${NC}"
    echo "Check logs: sudo journalctl -u ovs-port-agent -n 50"
  fi
else
  echo "=========================================="
  echo " Step 5: Service (Not Enabled)"
  echo "=========================================="
  echo ""
  echo "To enable later:"
  echo "  sudo systemctl enable --now ovs-port-agent"
fi

echo ""
echo "=========================================="
echo -e " ${GREEN}Installation Complete!${NC}"
echo "=========================================="
echo ""
echo "What's installed:"
echo "  ✓ Binary: ${BIN_DEST}"
echo "  ✓ Config: ${CONFIG_FILE}"
echo "  ✓ Ledger: ${LEDGER_DIR}"
echo "  ✓ Service: ${SYSTEMD_UNIT}"
echo "  ✓ D-Bus: ${DBUS_POLICY}"
echo "  ✓ Network: Applied ${NETWORK_CONFIG}"
echo "  ✓ Backup: ${BACKUP_DIR} (rollback available)"
echo ""
echo "Useful commands:"
echo "  sudo ovs-port-agent query-state --plugin network"
echo "  sudo ovs-port-agent apply-state <config.yaml>"
echo "  sudo ovs-vsctl show"
echo "  sudo systemctl status ovs-port-agent"
echo ""
echo "Documentation:"
echo "  docs/NETWORK_PLUGIN_GUIDE.md"
echo "  docs/STATE_MANAGER_ARCHITECTURE.md"
echo ""

