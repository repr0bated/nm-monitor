#!/usr/bin/env bash
set -euo pipefail

# Always run from repo root
SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

# Usage: ./scripts/install.sh [--bridge ovsbr0] [--with-ovsbr1] [--system] [--uplink IFACE] [--nm-ip CIDR] [--nm-gw GW] [--ovsbr1-ip CIDR] [--ovsbr1-gw GW] [--ovsbr1-uplink IFACE]
# - Installs ovs-port-agent binary, config, and systemd unit
# - Ensures OVS bridge exists (ovs-vsctl)
# - If NetworkManager is available, creates NM connections for the bridge (and optional uplink)
# - Optionally creates an empty OVS bridge ovsbr1

BRIDGE="ovsbr0"
WITH_OVSBR1=0
SYSTEM=0
PREFIX="/usr/local"
UPLINK=""
NM_IP=""
NM_GW=""
OVSBR1_IP=""
OVSBR1_GW=""
OVSBR1_UPLINK=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge)
      BRIDGE="$2"; shift 2;;
    --with-ovsbr1)
      WITH_OVSBR1=1; shift;;
    --system)
      SYSTEM=1; shift;;
    --uplink)
      UPLINK="$2"; shift 2;;
    --nm-ip)
      NM_IP="$2"; shift 2;;
    --nm-gw)
      NM_GW="$2"; shift 2;;
    --ovsbr1-ip)
      OVSBR1_IP="$2"; shift 2;;
    --ovsbr1-gw)
      OVSBR1_GW="$2"; shift 2;;
    --ovsbr1-uplink)
      OVSBR1_UPLINK="$2"; shift 2;;
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

# If NetworkManager is present, create NM connections for the bridge/uplink
if command -v nmcli >/dev/null 2>&1; then
  echo "Configuring NetworkManager connection for ${BRIDGE} (bridge + ovs-interface)"
  if ! nmcli -t -f NAME c show | grep -qx "${BRIDGE}"; then
    nmcli c add type ovs-bridge con-name "${BRIDGE}" ifname "${BRIDGE}"
  fi
  # Create an internal ovs-interface for L3 on the bridge
  if ! nmcli -t -f NAME c show | grep -qx "${BRIDGE}-if"; then
    nmcli c add type ovs-interface con-name "${BRIDGE}-if" ifname "${BRIDGE}" master "${BRIDGE}"
  fi
  if [[ -n "${NM_IP}" ]]; then
    nmcli c modify "${BRIDGE}-if" ipv4.method manual ipv4.addresses "${NM_IP}"
    if [[ -n "${NM_GW}" ]]; then
      nmcli c modify "${BRIDGE}-if" ipv4.gateway "${NM_GW}"
    fi
    nmcli c modify "${BRIDGE}-if" ipv6.method disabled || true
  fi
  if [[ -n "${UPLINK}" ]]; then
    PORT_NAME="${BRIDGE}-port-${UPLINK}"
    if ! nmcli -t -f NAME c show | grep -qx "${PORT_NAME}"; then
      nmcli c add type ovs-port con-name "${PORT_NAME}" ifname "${UPLINK}" master "${BRIDGE}"
    fi
    ETH_NAME="${BRIDGE}-uplink-${UPLINK}"
    if ! nmcli -t -f NAME c show | grep -qx "${ETH_NAME}"; then
      nmcli c add type ethernet con-name "${ETH_NAME}" ifname "${UPLINK}" master "${PORT_NAME}"
    fi
  fi
  nmcli c up "${BRIDGE}" || true
  nmcli c up "${BRIDGE}-if" || true
fi

# Optionally create ovsbr1
if [[ "$WITH_OVSBR1" == 1 ]]; then
  if ! ovs-vsctl br-exists ovsbr1; then
    echo "Creating OVS bridge ovsbr1"
    ovs-vsctl add-br ovsbr1
  fi
  if command -v nmcli >/dev/null 2>&1; then
    echo "Configuring NetworkManager connection for ovsbr1 (bridge + ovs-interface)"
    if ! nmcli -t -f NAME c show | grep -qx "ovsbr1"; then
      nmcli c add type ovs-bridge con-name "ovsbr1" ifname "ovsbr1"
    fi
    if ! nmcli -t -f NAME c show | grep -qx "ovsbr1-if"; then
      nmcli c add type ovs-interface con-name "ovsbr1-if" ifname "ovsbr1" master "ovsbr1"
    fi
    if [[ -n "${OVSBR1_IP}" ]]; then
      nmcli c modify "ovsbr1-if" ipv4.method manual ipv4.addresses "${OVSBR1_IP}"
      if [[ -n "${OVSBR1_GW}" ]]; then
        nmcli c modify "ovsbr1-if" ipv4.gateway "${OVSBR1_GW}"
      fi
      nmcli c modify "ovsbr1-if" ipv6.method disabled || true
    fi
    if [[ -n "${OVSBR1_UPLINK}" ]]; then
      PORT_NAME="ovsbr1-port-${OVSBR1_UPLINK}"
      if ! nmcli -t -f NAME c show | grep -qx "${PORT_NAME}"; then
        nmcli c add type ovs-port con-name "${PORT_NAME}" ifname "${OVSBR1_UPLINK}" master "ovsbr1"
      fi
      ETH_NAME="ovsbr1-uplink-${OVSBR1_UPLINK}"
      if ! nmcli -t -f NAME c show | grep -qx "${ETH_NAME}"; then
        nmcli c add type ethernet con-name "${ETH_NAME}" ifname "${OVSBR1_UPLINK}" master "${PORT_NAME}"
      fi
    fi
    nmcli c up "ovsbr1" || true
    nmcli c up "ovsbr1-if" || true
  fi
fi

echo "Done."
