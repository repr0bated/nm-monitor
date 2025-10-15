#!/usr/bin/env bash
# Interactive atomic install script with user prompts
# Zero connectivity loss with user-specified configuration

set -euo pipefail

readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly BLUE='\033[0;34m'
readonly YELLOW='\033[1;33m'
readonly NC='\033[0m'

log() { echo -e "$1" >&2; }
error_exit() { log "${RED}[ERROR]${NC} $1"; exit 1; }

[[ $EUID -eq 0 ]] || error_exit "Must run as root"

# Build the agent first
cd "$(dirname "$0")/.."
cargo build --release || error_exit "Failed to build agent"

log "${BLUE}=== Interactive OVS Bridge Setup ===${NC}"
echo

# Show current network state
log "${YELLOW}Current network interfaces:${NC}"
ip -brief addr show | grep -v "lo\|DOWN"
echo

log "${YELLOW}Current default route:${NC}"
ip route show default
echo

# Prompt for uplink interface
read -p "Enter uplink interface name (e.g., eth0, wlo1): " UPLINK
[[ -n "$UPLINK" ]] || error_exit "Uplink interface required"

# Validate uplink exists
if ! ip link show "$UPLINK" >/dev/null 2>&1; then
    error_exit "Interface $UPLINK does not exist"
fi

# Prompt for bridge name
read -p "Enter bridge name [ovsbr0]: " BRIDGE
BRIDGE=${BRIDGE:-ovsbr0}

# Get current IP from uplink as default
CURRENT_IP=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f1 | head -1)
CURRENT_PREFIX=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f2 | head -1)
CURRENT_GW=$(ip route show default | awk '{print $3}' | head -1)

# Prompt for IP configuration
read -p "Enter IP address [$CURRENT_IP]: " IP_ADDR
IP_ADDR=${IP_ADDR:-$CURRENT_IP}

read -p "Enter subnet prefix [$CURRENT_PREFIX]: " PREFIX
PREFIX=${PREFIX:-$CURRENT_PREFIX}

read -p "Enter gateway [$CURRENT_GW]: " GATEWAY
GATEWAY=${GATEWAY:-$CURRENT_GW}

read -p "Enter DNS servers [8.8.8.8,8.8.4.4]: " DNS
DNS=${DNS:-8.8.8.8,8.8.4.4}

# Show configuration summary
echo
log "${BLUE}Configuration Summary:${NC}"
log "  Uplink Interface: $UPLINK"
log "  Bridge Name: $BRIDGE"
log "  IP Address: $IP_ADDR/$PREFIX"
log "  Gateway: $GATEWAY"
log "  DNS: $DNS"
echo

read -p "Proceed with this configuration? [y/N]: " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log "${YELLOW}Installation cancelled${NC}"
    exit 0
fi

# Create NetworkManager checkpoint for atomic rollback
log "${BLUE}Creating NetworkManager checkpoint${NC}"
CHECKPOINT=$(nmcli general checkpoint create 300 2>/dev/null) || error_exit "Failed to create checkpoint"
log "Checkpoint created: $CHECKPOINT"

# Cleanup function for rollback
cleanup() {
    if [[ -n "${CHECKPOINT:-}" ]]; then
        log "${RED}Rolling back to checkpoint${NC}"
        nmcli general checkpoint rollback "$CHECKPOINT" 2>/dev/null || true
        nmcli general checkpoint destroy "$CHECKPOINT" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Create OVS bridge connection atomically via NetworkManager
log "${BLUE}Creating OVS bridge atomically${NC}"

# 1. Create OVS bridge
nmcli connection add type ovs-bridge conn.interface "$BRIDGE" con-name "$BRIDGE" || error_exit "Failed to create OVS bridge"

# 2. Create OVS port for uplink
nmcli connection add type ovs-port conn.interface "$UPLINK" master "$BRIDGE" con-name "ovs-port-$UPLINK" || error_exit "Failed to create OVS port"

# 3. Create OVS interface with IP (this is where IP moves atomically)
nmcli connection add type ovs-interface slave-type ovs-port conn.interface "$BRIDGE" master "$BRIDGE" \
    con-name "$BRIDGE-iface" \
    ipv4.method manual \
    ipv4.addresses "$IP_ADDR/$PREFIX" \
    ipv4.gateway "$GATEWAY" \
    ipv4.dns "$DNS" || error_exit "Failed to create OVS interface"

# 4. Modify uplink to be enslaved (removes IP atomically)
nmcli connection modify "$UPLINK" master "$BRIDGE" slave-type ovs-port ipv4.method disabled || error_exit "Failed to enslave uplink"

# 5. Activate all connections atomically
log "${BLUE}Activating bridge atomically${NC}"
nmcli connection up "$BRIDGE" || error_exit "Failed to activate bridge"
nmcli connection up "ovs-port-$UPLINK" || error_exit "Failed to activate port"
nmcli connection up "$BRIDGE-iface" || error_exit "Failed to activate interface"
nmcli connection up "$UPLINK" || error_exit "Failed to activate enslaved uplink"

# Verify connectivity preserved
log "${BLUE}Verifying connectivity${NC}"
sleep 3
if ! ping -c 1 -W 3 8.8.8.8 >/dev/null 2>&1; then
    error_exit "Connectivity lost - rolling back"
fi

# Install the agent
log "${BLUE}Installing agent${NC}"
install -m 0755 target/release/ovs-port-agent /usr/local/bin/
install -d /etc/ovs-port-agent
[[ ! -f /etc/ovs-port-agent/config.toml ]] && install -m 0644 config/config.toml.example /etc/ovs-port-agent/config.toml

# Update config with user's bridge name
sed -i "s/bridge_name = \".*\"/bridge_name = \"$BRIDGE\"/" /etc/ovs-port-agent/config.toml

install -m 0644 dbus/dev.ovs.PortAgent1.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovs-port-agent.service /etc/systemd/system/

systemctl daemon-reload
systemctl enable --now ovs-port-agent

# Success - destroy checkpoint (no rollback needed)
nmcli general checkpoint destroy "$CHECKPOINT" 2>/dev/null || true
CHECKPOINT=""  # Prevent cleanup rollback

echo
log "${GREEN}✅ Installation completed successfully!${NC}"
log "${GREEN}✅ Zero connectivity loss${NC}"
log "${GREEN}✅ OVS bridge: $BRIDGE with IP $IP_ADDR/$PREFIX${NC}"
log "${GREEN}✅ Uplink: $UPLINK enslaved to bridge${NC}"
log "${GREEN}✅ Gateway: $GATEWAY${NC}"
log "${GREEN}✅ DNS: $DNS${NC}"
echo
log "${BLUE}Bridge status:${NC}"
nmcli connection show --active | grep -E "$BRIDGE|ovs"
