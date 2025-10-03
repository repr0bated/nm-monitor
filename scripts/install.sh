#!/usr/bin/env bash
set -euo pipefail

# Always run from repo root
SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

# Usage: ./scripts/install.sh [--bridge ovsbr0] [--with-ovsbr1] [--system]
# - Installs ovs-port-agent binary, config, and systemd unit
# - Optionally creates an empty OVS bridge ovsbr1

BRIDGE="ovsbr0"
WITH_OVSBR1=0
SYSTEM=0
PREFIX="/usr/local"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge)
      BRIDGE="$2"; shift 2;;
    --with-ovsbr1)
      WITH_OVSBR1=1; shift;;
    --system)
      SYSTEM=1; shift;;
    *) echo "Unknown arg: $1"; exit 1;;
  esac
done

# Ensure dependencies
command -v ovs-vsctl >/dev/null || { echo "ERROR: ovs-vsctl not found"; exit 2; }
command -v install >/dev/null || { echo "ERROR: install not found"; exit 2; }

# Build release binary
echo "Building release binary..."
cargo build --release

echo "Installing binary to ${PREFIX}/bin";
install -m 0755 target/release/ovs-port-agent "${PREFIX}/bin/"

# Config
mkdir -p /etc/ovs-port-agent
if [[ ! -f /etc/ovs-port-agent/config.toml ]]; then
  install -m 0644 config/config.toml.example /etc/ovs-port-agent/config.toml
  sed -i "s/^bridge_name = \".*\"/bridge_name = \"${BRIDGE}\"/" /etc/ovs-port-agent/config.toml
  # Ensure renaming is enabled by default
  if ! grep -q '^enable_rename' /etc/ovs-port-agent/config.toml; then
    printf '\n# Enable renaming by default\n' >> /etc/ovs-port-agent/config.toml
    printf 'enable_rename = true\n' >> /etc/ovs-port-agent/config.toml
  else
    sed -i 's/^enable_rename.*/enable_rename = true/' /etc/ovs-port-agent/config.toml
  fi
fi

# Systemd unit
install -m 0644 systemd/ovs-port-agent.service /etc/systemd/system/ovs-port-agent.service
systemctl daemon-reload

if [[ "$SYSTEM" == 1 ]]; then
  systemctl enable --now ovs-port-agent
  systemctl status --no-pager ovs-port-agent || true
else
  echo "Unit installed. Start with: systemctl enable --now ovs-port-agent"
fi

# Create base bridge if missing
if ! ovs-vsctl br-exists "${BRIDGE}"; then
  echo "Creating OVS bridge ${BRIDGE}"
  ovs-vsctl add-br "${BRIDGE}"
fi

# Optionally create ovsbr1
if [[ "$WITH_OVSBR1" == 1 ]]; then
  if ! ovs-vsctl br-exists ovsbr1; then
    echo "Creating OVS bridge ovsbr1"
    ovs-vsctl add-br ovsbr1
  fi
fi

echo "Done."
