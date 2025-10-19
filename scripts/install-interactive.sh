#!/usr/bin/env bash
# Interactive atomic install script - only prompts for uplink, introspects everything else
# Zero connectivity loss with automatic configuration detection

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

# Prompt for uplink interface ONLY
read -p "Enter uplink interface name (e.g., eth0, wlo1): " UPLINK
[[ -n "$UPLINK" ]] || error_exit "Uplink interface required"

# Validate uplink exists
if ! ip link show "$UPLINK" >/dev/null 2>&1; then
    error_exit "Interface $UPLINK does not exist"
fi

# INTROSPECT everything else
log "${BLUE}Introspecting network configuration...${NC}"

# Bridge name - always ovsbr0
BRIDGE="ovsbr0"

# Get current IP configuration from uplink
IP_ADDR=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f1 | head -1)
PREFIX=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f2 | head -1)
GATEWAY=$(ip route show default | awk '{print $3}' | head -1)

# Validate introspected values, prompt only if missing
if [[ -z "$IP_ADDR" ]]; then
    read -p "Could not detect IP address. Enter IP: " IP_ADDR
    [[ -n "$IP_ADDR" ]] || error_exit "IP address required"
fi

if [[ -z "$PREFIX" ]]; then
    read -p "Could not detect subnet prefix. Enter prefix (e.g., 24): " PREFIX
    [[ -n "$PREFIX" ]] || error_exit "Subnet prefix required"
fi

if [[ -z "$GATEWAY" ]]; then
    read -p "Could not detect gateway. Enter gateway IP: " GATEWAY
    [[ -n "$GATEWAY" ]] || error_exit "Gateway required"
fi

# DNS - always use reliable defaults
DNS="8.8.8.8,8.8.4.4"

# Show introspected configuration
echo
log "${BLUE}Detected Configuration:${NC}"
log "  Uplink Interface: $UPLINK (user specified)"
log "  Bridge Name: $BRIDGE (default)"
log "  IP Address: $IP_ADDR/$PREFIX (introspected)"
log "  Gateway: $GATEWAY (introspected)"
log "  DNS: $DNS (default)"
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
echo
log "${BLUE}Bridge status:${NC}"
nmcli connection show --active | grep -E "$BRIDGE|ovs"
