#!/usr/bin/env bash
set -euo pipefail

# Configure ovsbr0 and ovsbr1 via the installer (NM-aligned OVS topology)
# Override via environment variables

: "${BRIDGE:=ovsbr0}"
: "${NM_IP:=80.209.240.244/25}"
: "${NM_GW:=80.209.240.129}"
: "${UPLINK:=}"

: "${OVSBR1:=yes}"
: "${OVSBR1_IP:=10.200.0.1/24}"
: "${OVSBR1_GW:=}"
: "${OVSBR1_UPLINK:=}"

# Optional: add a secondary public IP to ovsbr0 connection
: "${SECONDARY_IP:=}"

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

INSTALL_ARGS=(
  --bridge "$BRIDGE" --nm-ip "$NM_IP" --nm-gw "$NM_GW" --system
)

if [[ -n "$UPLINK" ]]; then
  INSTALL_ARGS+=( --uplink "$UPLINK" )
fi

if [[ "${OVSBR1}" == "yes" ]]; then
  INSTALL_ARGS+=( --with-ovsbr1 --ovsbr1-ip "$OVSBR1_IP" )
  [[ -n "$OVSBR1_GW" ]] && INSTALL_ARGS+=( --ovsbr1-gw "$OVSBR1_GW" )
  [[ -n "$OVSBR1_UPLINK" ]] && INSTALL_ARGS+=( --ovsbr1-uplink "$OVSBR1_UPLINK" )
fi

sudo ./scripts/install.sh "${INSTALL_ARGS[@]}"

if [[ -n "$SECONDARY_IP" ]]; then
  CONN_NAME="$BRIDGE"
  echo "Adding secondary IP $SECONDARY_IP to $CONN_NAME"
  CONN="$CONN_NAME" IPADDR="$SECONDARY_IP" sudo -E ./scripts/add_secondary_ip.sh
fi
