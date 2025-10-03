#!/usr/bin/env bash
set -euo pipefail

# Configure ovsbr0 and ovsbr1 via the installer
# Override via env or CLI vars
: "${BRIDGE:=ovsbr0}"
: "${NM_IP:=80.209.240.244/25}"
: "${NM_GW:=80.209.240.129}"
: "${OVSBR1_IP:=10.200.0.1/24}"

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

sudo ./scripts/install.sh \
  --bridge "$BRIDGE" --nm-ip "$NM_IP" --nm-gw "$NM_GW" \
  --with-ovsbr1 --ovsbr1-ip "$OVSBR1_IP" \
  --system
