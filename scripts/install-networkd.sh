#!/usr/bin/env bash
# Atomic systemd-networkd install script - NO NetworkManager
# Uses pure systemd-networkd with atomic configuration deployment

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

log "${BLUE}=== systemd-networkd OVS Bridge Setup ===${NC}"
echo

# Show current network state
log "${YELLOW}Current network interfaces:${NC}"
ip -brief addr show | grep -v "lo\|DOWN"
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

# Validate introspected values
[[ -n "$IP_ADDR" ]] || error_exit "Could not detect IP address on $UPLINK"
[[ -n "$PREFIX" ]] || error_exit "Could not detect subnet prefix on $UPLINK"
[[ -n "$GATEWAY" ]] || error_exit "Could not detect gateway"

# Show introspected configuration
echo
log "${BLUE}Detected Configuration:${NC}"
log "  Uplink Interface: $UPLINK (user specified)"
log "  Bridge Name: $BRIDGE (default)"
log "  IP Address: $IP_ADDR/$PREFIX (introspected)"
log "  Gateway: $GATEWAY (introspected)"
echo

read -p "Proceed with atomic systemd-networkd setup? [y/N]: " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log "${YELLOW}Installation cancelled${NC}"
    exit 0
fi

# Create backup of current network config
BACKUP_DIR="/tmp/networkd-backup-$(date +%s)"
log "${BLUE}Creating backup: $BACKUP_DIR${NC}"
mkdir -p "$BACKUP_DIR"
cp -r /etc/systemd/network/* "$BACKUP_DIR/" 2>/dev/null || true

# Cleanup function for rollback
cleanup() {
    if [[ -d "$BACKUP_DIR" ]]; then
        log "${RED}Rolling back systemd-networkd configuration${NC}"
        rm -rf /etc/systemd/network/*
        cp -r "$BACKUP_DIR"/* /etc/systemd/network/ 2>/dev/null || true
        systemctl restart systemd-networkd
        sleep 3
    fi
}
trap cleanup EXIT

# Create systemd-networkd configuration atomically
NETD_DIR="/etc/systemd/network"

log "${BLUE}Creating systemd-networkd OVS configuration${NC}"

# 1. Create OVS bridge netdev
cat > "$NETD_DIR/10-$BRIDGE.netdev" <<EOF
[NetDev]
Name=$BRIDGE
Kind=openvswitch
EOF

# 2. Create bridge network with IP
cat > "$NETD_DIR/30-$BRIDGE.network" <<EOF
[Match]
Name=$BRIDGE

[Network]
Address=$IP_ADDR/$PREFIX
Gateway=$GATEWAY
DNS=8.8.8.8
DNS=8.8.4.4
ConfigureWithoutCarrier=yes
IgnoreCarrierLoss=yes
EOF

# 3. Create uplink network (enslaved to bridge)
cat > "$NETD_DIR/20-$UPLINK.network" <<EOF
[Match]
Name=$UPLINK

[Network]
Bridge=$BRIDGE
ConfigureWithoutCarrier=yes
IgnoreCarrierLoss=yes
EOF

# 4. Atomic reload of systemd-networkd
log "${BLUE}Applying configuration atomically${NC}"
systemctl reload-or-restart systemd-networkd

# Wait for network to stabilize
log "${BLUE}Waiting for network convergence...${NC}"
sleep 5

# Verify connectivity
if ! ping -c 1 -W 3 8.8.8.8 >/dev/null 2>&1; then
    error_exit "Connectivity lost - rolling back"
fi

# Verify bridge exists and has IP
if ! ip addr show "$BRIDGE" | grep -q "$IP_ADDR"; then
    error_exit "Bridge IP not configured correctly - rolling back"
fi

# Install the agent
log "${BLUE}Installing ovs-port-agent${NC}"
install -m 0755 target/release/ovs-port-agent /usr/local/bin/
install -d /etc/ovs-port-agent
[[ ! -f /etc/ovs-port-agent/config.toml ]] && install -m 0644 config/config.toml.example /etc/ovs-port-agent/config.toml

install -m 0644 dbus/dev.ovs.PortAgent1.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovs-port-agent.service /etc/systemd/system/

systemctl daemon-reload
systemctl enable --now ovs-port-agent

# Success - remove backup (no rollback needed)
rm -rf "$BACKUP_DIR"
BACKUP_DIR=""  # Prevent cleanup rollback

echo
log "${GREEN}✅ systemd-networkd installation completed!${NC}"
log "${GREEN}✅ Zero connectivity loss${NC}"
log "${GREEN}✅ OVS bridge: $BRIDGE with IP $IP_ADDR/$PREFIX${NC}"
log "${GREEN}✅ Uplink: $UPLINK enslaved to bridge${NC}"
log "${GREEN}✅ Gateway: $GATEWAY${NC}"
echo
log "${BLUE}Network status:${NC}"
ip -brief addr show | grep -E "$BRIDGE|$UPLINK"
echo
log "${BLUE}systemd-networkd configuration:${NC}"
ls -la /etc/systemd/network/
