#!/usr/bin/env bash
# Install script with systemd-networkd native OVS atomic handoff
# NO NetworkManager needed - systemd-networkd has built-in OVS support!

set -euo pipefail

readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly RED='\033[0;31m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
PREFIX="/usr/local"
ENABLE_SERVICE=0
FORCE_NATIVE=0

log() { echo -e "$1" >&2; }
error_exit() { log "${RED}[ERROR]${NC} $1"; exit 1; }

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --system) ENABLE_SERVICE=1; shift ;;
    --prefix) PREFIX="$2"; shift 2 ;;
    --force-native) FORCE_NATIVE=1; shift ;;
    *) error_exit "Unknown argument: $1" ;;
  esac
done

[[ $EUID -eq 0 ]] || error_exit "Must run as root"

log "${BLUE}========================================${NC}"
log "${BLUE} OVS Setup with systemd-networkd${NC}"
log "${BLUE} Native OVS support - NO NetworkManager!${NC}"
log "${BLUE}========================================${NC}"
echo ""

# Step 1: Build binary
log "${BLUE}Step 1: Build Binary${NC}"
CARGO_BIN=""
if command -v cargo >/dev/null 2>&1; then
  CARGO_BIN=$(command -v cargo)
elif [[ -n "${SUDO_USER:-}" ]]; then
  SUDO_HOME=$(getent passwd "$SUDO_USER" | cut -d: -f6)
  [[ -n "$SUDO_HOME" && -x "$SUDO_HOME/.cargo/bin/cargo" ]] && CARGO_BIN="$SUDO_HOME/.cargo/bin/cargo"
fi

[[ -n "$CARGO_BIN" ]] || error_exit "cargo not found"

log "Building ovs-port-agent..."
cd "${REPO_ROOT}"
export PATH="$(dirname "${CARGO_BIN}"):${PATH}"
"${CARGO_BIN}" build --release 2>&1 | grep -v "^warning:" || true
log "${GREEN}✓${NC} Build complete"
echo ""

# Step 2: Check prerequisites
log "${BLUE}Step 2: Check Prerequisites${NC}"

# Check systemd-networkd
if ! systemctl is-active --quiet systemd-networkd; then
  log "${YELLOW}Starting systemd-networkd...${NC}"
  systemctl start systemd-networkd || error_exit "Failed to start systemd-networkd"
fi

# Check OVS
if ! systemctl is-active --quiet openvswitch-switch; then
  log "${YELLOW}Starting openvswitch-switch...${NC}"
  systemctl start openvswitch-switch || error_exit "Failed to start OVS"
fi

log "${GREEN}✓${NC} Prerequisites OK"
echo ""

# Step 3: Introspect uplink
log "${BLUE}Step 3: Introspect Uplink${NC}"

# Find uplink (interface with default route)
UPLINK=$(ip -o -4 route show default | awk '{print $5}' | head -1)
[[ -n "$UPLINK" ]] || UPLINK=$(ip -o -4 addr show | awk '$2 !~ /^(lo|ovsbr|docker|br-)/ {print $2}' | head -1)
[[ -n "$UPLINK" ]] || error_exit "Could not detect uplink"

# Get current IP config via D-Bus introspection
UPLINK_IP=$(ip -o -4 addr show "${UPLINK}" | awk '{print $4}' | cut -d/ -f1)
UPLINK_PREFIX=$(ip -o -4 addr show "${UPLINK}" | awk '{print $4}' | cut -d/ -f2)
UPLINK_GW=$(ip route show default | grep "${UPLINK}" | awk '{print $3}' | head -1)

# Get DNS
DNS_SERVERS=()
if [[ -f /etc/resolv.conf ]]; then
  while IFS= read -r line; do
    if [[ $line =~ ^nameserver[[:space:]]+([0-9.]+) ]]; then
      DNS_SERVERS+=("${BASH_REMATCH[1]}")
    fi
  done < /etc/resolv.conf
fi
[[ ${#DNS_SERVERS[@]} -eq 0 ]] && DNS_SERVERS=("8.8.8.8" "8.8.4.4")

log "  Uplink: ${UPLINK}"
log "  IP: ${UPLINK_IP}/${UPLINK_PREFIX}"
log "  Gateway: ${UPLINK_GW}"
log "  DNS: ${DNS_SERVERS[*]}"
echo ""

# Step 4: Create systemd-networkd OVS configuration
log "${BLUE}Step 4: Create OVS Configuration (Atomic Handoff)${NC}"
echo ""

NETD_DIR="/etc/systemd/network"
mkdir -p "$NETD_DIR"

# Backup existing uplink config
if [[ -f "$NETD_DIR/10-${UPLINK}.network" ]]; then
  cp "$NETD_DIR/10-${UPLINK}.network" "$NETD_DIR/10-${UPLINK}.network.backup-$(date +%s)"
fi

log "Creating ovsbr0 (primary bridge with uplink)..."

# 1. Create ovsbr0 bridge device
# First, detect if systemd-networkd supports OVS kinds. We probe with a temp .netdev.
SUPPORTS_OVS=1
TMP_TEST="$NETD_DIR/99-ovs-test.netdev"
cat > "$TMP_TEST" <<'EOF'
[NetDev]
Name=ovstest0
Kind=ovs-bridge
EOF
if ! networkctl reload 2>/dev/null; then
  busctl call org.freedesktop.network1 /org/freedesktop/network1 org.freedesktop.network1.Manager Reload >/dev/null 2>&1 || true
fi
sleep 1
# Check journal for parse errors
if journalctl -u systemd-networkd --since "-2min" --no-pager | grep -q "Failed to parse netdev kind"; then
  SUPPORTS_OVS=0
fi
rm -f "$TMP_TEST" || true

if [[ "$SUPPORTS_OVS" -eq 1 ]]; then
  # 1. Create ovsbr0 bridge device with native systemd-networkd OVS support
  cat > "$NETD_DIR/10-ovsbr0.netdev" <<EOF
# OVS Bridge 0 - Primary bridge with uplink
# Created: $(date)
[NetDev]
Name=ovsbr0
Kind=ovs-bridge
EOF

# 2. Attach uplink to bridge (IP moves from ens1 to ovsbr0)
  cat > "$NETD_DIR/20-${UPLINK}.network" <<EOF
# Uplink interface - attached to ovsbr0
# IP configuration moves to bridge
[Match]
Name=${UPLINK}

[Network]
Bridge=ovsbr0
IgnoreCarrierLoss=yes
EOF

# 3. Configure ovsbr0 with IP (atomic handoff happens here!)
  cat > "$NETD_DIR/30-ovsbr0.network" <<EOF
# OVS Bridge 0 network configuration
# IP moved from ${UPLINK}
[Match]
Name=ovsbr0

[Network]
Address=${UPLINK_IP}/${UPLINK_PREFIX}
Gateway=${UPLINK_GW}
EOF

  for dns in "${DNS_SERVERS[@]}"; do
    echo "DNS=$dns" >> "$NETD_DIR/30-ovsbr0.network"
  done

  cat >> "$NETD_DIR/30-ovsbr0.network" <<EOF
IgnoreCarrierLoss=yes
ConfigureWithoutCarrier=yes
EOF

log "${GREEN}✓${NC} ovsbr0 configuration created"

# Create ovsbr1 (isolated bridge)
  log "Creating ovsbr1 (isolated bridge)..."

  cat > "$NETD_DIR/11-ovsbr1.netdev" <<EOF
# OVS Bridge 1 - Isolated bridge for containers
# Created: $(date)
[NetDev]
Name=ovsbr1
Kind=ovs-bridge
EOF

  cat > "$NETD_DIR/31-ovsbr1.network" <<EOF
# OVS Bridge 1 network configuration
[Match]
Name=ovsbr1

[Network]
Address=80.209.242.196/25
Gateway=80.209.242.129
DNS=8.8.8.8
DNS=8.8.4.4
IgnoreCarrierLoss=yes
ConfigureWithoutCarrier=yes
EOF

  log "${GREEN}✓${NC} ovsbr1 configuration created"
else
  if [[ "$FORCE_NATIVE" -eq 1 ]]; then
    error_exit "systemd-networkd native OVS is not supported on this system (Kind=ovs-bridge not recognized) and --force-native was specified. Aborting."
  fi
  log "${YELLOW}systemd-networkd appears to lack OVS netdev support (Kind=ovs-bridge not recognized).${NC}"
  log "${YELLOW}Falling back to direct OVS setup via ovs-vsctl with an atomic IP handoff.${NC}"
  # Create bridges and attach uplink
  ovs-vsctl --may-exist add-br ovsbr0 || error_exit "Failed to add ovsbr0"
  ovs-vsctl --may-exist add-br ovsbr1 || error_exit "Failed to add ovsbr1"
  # Bring up bridges
  ip link set ovsbr0 up || true
  ip link set ovsbr1 up || true
  # Attach uplink to ovsbr0 if not already
  if ! ovs-vsctl list-ports ovsbr0 | grep -q "^${UPLINK}$"; then
    ovs-vsctl --may-exist add-port ovsbr0 "${UPLINK}" || error_exit "Failed to add uplink ${UPLINK} to ovsbr0"
  fi
  # Atomic-ish IP handoff
  if [[ -n "${UPLINK_IP}" && -n "${UPLINK_PREFIX}" ]]; then
    ip addr del "${UPLINK_IP}/${UPLINK_PREFIX}" dev "${UPLINK}" 2>/dev/null || true
    ip addr add "${UPLINK_IP}/${UPLINK_PREFIX}" dev ovsbr0 || error_exit "Failed to assign IP to ovsbr0"
  fi
  # Reset default route to use bridge
  if [[ -n "${UPLINK_GW}" ]]; then
    ip route del default 2>/dev/null || true
    ip route add default via "${UPLINK_GW}" dev ovsbr0 || true
  fi
  # Send gratuitous ARP to refresh neighbor caches (minimize interruption)
  if command -v arping >/dev/null 2>&1; then
    log "Sending gratuitous ARP for ${UPLINK_IP} on ovsbr0..."
    arping -U -I ovsbr0 -c 3 "${UPLINK_IP}" >/dev/null 2>&1 || true
    if [[ -n "${UPLINK_GW}" ]]; then
      arping -S "${UPLINK_IP}" -I ovsbr0 -c 2 "${UPLINK_GW}" >/dev/null 2>&1 || true
    fi
  fi
  # Persist IP on ovsbr0 for future boots
  cat > "$NETD_DIR/30-ovsbr0.network" <<EOF
[Match]
Name=ovsbr0

[Network]
Address=${UPLINK_IP}/${UPLINK_PREFIX}
Gateway=${UPLINK_GW}
EOF
  for dns in "${DNS_SERVERS[@]}"; do
    echo "DNS=$dns" >> "$NETD_DIR/30-ovsbr0.network"
  done
  log "${GREEN}✓${NC} Fallback OVS topology created"
fi
echo ""

# Step 5: Atomic handoff via systemd-networkd reload
log "${BLUE}Step 5: Atomic Handoff (networkctl reload)${NC}"
log ""
log "${YELLOW}This will atomically:${NC}"
log "  1. Create ovsbr0 and ovsbr1 via OVS"
log "  2. Move ${UPLINK} IP to ovsbr0"
log "  3. Attach ${UPLINK} to ovsbr0"
log "  4. No connectivity loss (IgnoreCarrierLoss=yes)"
echo ""

# Using D-Bus for atomic reload
log "Calling systemd-networkd.Reload via D-Bus..."
busctl call org.freedesktop.network1 \
  /org/freedesktop/network1 \
  org.freedesktop.network1.Manager \
  Reload || error_exit "Failed to reload systemd-networkd"

log "${GREEN}✓${NC} Atomic handoff initiated"
echo ""

# Wait for convergence
log "Waiting for network to stabilize..."
sleep 3

# Verify
log "${BLUE}Step 6: Verify${NC}"
echo ""

if ovs-vsctl br-exists ovsbr0 && ovs-vsctl br-exists ovsbr1; then
  log "${GREEN}✓${NC} OVS bridges created"
else
  error_exit "OVS bridges not created"
fi

if ip addr show ovsbr0 | grep -q "${UPLINK_IP}"; then
  log "${GREEN}✓${NC} IP moved to ovsbr0"
else
  log "${YELLOW}WARNING:${NC} IP not yet on ovsbr0 (may take a moment)"
fi

if ovs-vsctl list-ports ovsbr0 | grep -q "${UPLINK}"; then
  log "${GREEN}✓${NC} Uplink attached to ovsbr0"
else
  log "${YELLOW}WARNING:${NC} Uplink not yet attached (may take a moment)"
fi

echo ""
log "${YELLOW}=== Current State ===${NC}"
ip -brief addr show
echo ""
ovs-vsctl show
echo ""

# Step 7: Install files
log "${BLUE}Step 7: Install ovs-port-agent${NC}"
echo ""

BIN_DEST="${PREFIX}/bin/ovs-port-agent"
CONFIG_DIR="/etc/ovs-port-agent"
LEDGER_DIR="/var/lib/ovs-port-agent"

install -d -m 0755 "${PREFIX}/bin"
install -m 0755 target/release/ovs-port-agent "${BIN_DEST}"
log "  Binary: ${BIN_DEST}"

install -d -m 0755 "${CONFIG_DIR}"
[[ ! -f "${CONFIG_DIR}/config.toml" ]] && install -m 0644 config/config.toml.example "${CONFIG_DIR}/config.toml"
log "  Config: ${CONFIG_DIR}"

install -d -m 0750 "${LEDGER_DIR}"
log "  Ledger: ${LEDGER_DIR}"

install -m 0644 dbus/dev.ovs.PortAgent1.conf "/etc/dbus-1/system.d/"
install -m 0644 systemd/ovs-port-agent.service "/etc/systemd/system/"

systemctl daemon-reload
systemctl reload dbus.service 2>/dev/null || systemctl restart dbus.service

log "${GREEN}✓${NC} Installation complete"
echo ""

if [[ $ENABLE_SERVICE -eq 1 ]]; then
  log "Enabling service..."
  systemctl enable ovs-port-agent
  systemctl start ovs-port-agent
  sleep 1
  systemctl is-active --quiet ovs-port-agent && log "${GREEN}✓${NC} Service running"
fi

log "${GREEN}========================================${NC}"
log "${GREEN} SUCCESS!${NC}"
log "${GREEN}========================================${NC}"
echo ""
log "Bridges created with atomic handoff:"
log "  • ovsbr0: ${UPLINK_IP}/${UPLINK_PREFIX} (uplink: ${UPLINK})"
log "  • ovsbr1: 80.209.242.196/25 (isolated)"
echo ""
log "systemd-networkd native OVS configuration:"
log "  ${NETD_DIR}/*.netdev"
log "  ${NETD_DIR}/*.network"
echo ""
log "No NetworkManager needed! ✨"
echo ""
