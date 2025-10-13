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

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --network-config)
      [[ $# -ge 2 ]] || { echo "Missing value for --network-config" >&2; exit 1; }
      NETWORK_CONFIG="$2"
      shift 2
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
  --network-config FILE   Path to network config YAML (required)
  --system                Enable and start systemd service after install
  --prefix DIR            Installation prefix (default: /usr/local)
  --help                  Show this help

Example:
  sudo $0 --network-config config/examples/network-ovs-bridges.yaml --system

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

# Validate network config
if [[ -z "${NETWORK_CONFIG}" ]]; then
  echo -e "${RED}ERROR: --network-config is required${NC}" >&2
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

echo "Configuration:"
echo "  Network Config: ${NETWORK_CONFIG}"
echo "  Install Prefix: ${PREFIX}"
echo "  Enable Service: $([ ${ENABLE_SERVICE} -eq 1 ] && echo 'Yes' || echo 'No')"
echo ""

# Check prerequisites
echo "Checking prerequisites..."

if ! command -v cargo >/dev/null 2>&1; then
  echo -e "${RED}ERROR: cargo not found${NC}" >&2
  echo "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

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
cargo build --release

echo ""
echo -e "${GREEN}✓${NC} Build complete"
echo ""

# Install files
echo "=========================================="
echo " Step 2: Installing Files"
echo "=========================================="
echo ""

BIN_DEST="${PREFIX}/bin/ovs-port-agent"
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

# Apply network configuration
echo "=========================================="
echo " Step 3: Apply Network Configuration"
echo "=========================================="
echo ""

echo "Config: ${NETWORK_CONFIG}"
echo ""

# Show diff first
echo "Calculating changes (dry run)..."
echo "---"
"${BIN_DEST}" show-diff "${NETWORK_CONFIG}" || {
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
  echo "To apply later: sudo ovs-port-agent apply-state ${NETWORK_CONFIG}"
  exit 0
fi

echo "Applying network configuration..."
"${BIN_DEST}" apply-state "${NETWORK_CONFIG}" || {
  echo -e "${RED}ERROR: Failed to apply network config${NC}" >&2
  echo ""
  echo "Troubleshooting:"
  echo "  1. Check OVS is running: sudo systemctl status openvswitch-switch"
  echo "  2. Check config syntax: sudo ovs-port-agent show-diff ${NETWORK_CONFIG}"
  echo "  3. Check logs: sudo journalctl -xe"
  exit 1
}

echo ""
echo -e "${GREEN}✓${NC} Network configuration applied"
echo ""

# Verify
echo "Verifying network state..."
echo "---"
"${BIN_DEST}" query-state --plugin network | head -40 || true
echo "---"
echo ""

# Enable service
if [[ ${ENABLE_SERVICE} -eq 1 ]]; then
  echo "=========================================="
  echo " Step 4: Enable Service"
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
  echo " Step 4: Service (Not Enabled)"
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

