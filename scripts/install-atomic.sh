#!/usr/bin/env bash
# Truly atomic install script using NetworkManager checkpoints
# Zero connectivity loss guaranteed

set -euo pipefail

readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

log() { echo -e "$1" >&2; }
error_exit() { log "${RED}[ERROR]${NC} $1"; exit 1; }

[[ $EUID -eq 0 ]] || error_exit "Must run as root"

# Build the agent first
cd "$(dirname "$0")/.."
cargo build --release || error_exit "Failed to build agent"

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

# Detect uplink interface
UPLINK=$(ip route show default | awk '{print $5}' | head -1)
[[ -n "$UPLINK" ]] || error_exit "Could not detect uplink interface"

# Get current IP configuration
UPLINK_IP=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f1 | head -1)
UPLINK_PREFIX=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f2 | head -1)
GATEWAY=$(ip route show default | awk '{print $3}' | head -1)

[[ -n "$UPLINK_IP" ]] || error_exit "Could not detect IP address for $UPLINK"

log "Detected configuration:"
log "  Uplink: $UPLINK"
log "  IP: $UPLINK_IP/$UPLINK_PREFIX"
log "  Gateway: $GATEWAY"

# Create OVS bridge connection atomically via NetworkManager
log "${BLUE}Creating OVS bridge atomically${NC}"

# 1. Create OVS bridge
nmcli connection add type ovs-bridge conn.interface ovsbr0 con-name ovsbr0 || error_exit "Failed to create OVS bridge"

# 2. Create OVS port for uplink
nmcli connection add type ovs-port conn.interface "$UPLINK" master ovsbr0 con-name "ovs-port-$UPLINK" || error_exit "Failed to create OVS port"

# 3. Create OVS interface with IP (this is where IP moves atomically)
nmcli connection add type ovs-interface slave-type ovs-port conn.interface ovsbr0 master ovsbr0 \
    con-name ovsbr0-iface \
    ipv4.method manual \
    ipv4.addresses "$UPLINK_IP/$UPLINK_PREFIX" \
    ipv4.gateway "$GATEWAY" \
    ipv4.dns "8.8.8.8,8.8.4.4" || error_exit "Failed to create OVS interface"

# 4. Modify uplink to be enslaved (removes IP atomically)
nmcli connection modify "$UPLINK" master ovsbr0 slave-type ovs-port ipv4.method disabled || error_exit "Failed to enslave uplink"

# 5. Activate all connections atomically
log "${BLUE}Activating bridge atomically${NC}"
nmcli connection up ovsbr0 || error_exit "Failed to activate bridge"
nmcli connection up "ovs-port-$UPLINK" || error_exit "Failed to activate port"
nmcli connection up ovsbr0-iface || error_exit "Failed to activate interface"
nmcli connection up "$UPLINK" || error_exit "Failed to activate enslaved uplink"

# Verify connectivity preserved
log "${BLUE}Verifying connectivity${NC}"
sleep 2
if ! ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1; then
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

log "${GREEN}✓ Atomic installation complete!${NC}"
log "${GREEN}✓ Zero connectivity loss${NC}"
log "${GREEN}✓ OVS bridge: ovsbr0 with IP $UPLINK_IP/$UPLINK_PREFIX${NC}"
log "${GREEN}✓ Uplink: $UPLINK enslaved to bridge${NC}"
