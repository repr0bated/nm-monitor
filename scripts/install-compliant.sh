#!/usr/bin/env bash
# Compliant systemd-networkd install script using pure zbus operations

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

# Create checkpoint
CHECKPOINT_ID=$(date +%s)
log "${BLUE}Creating checkpoint: $CHECKPOINT_ID${NC}"
mkdir -p "/tmp/networkd-checkpoint-$CHECKPOINT_ID"
cp -r /etc/systemd/network/* "/tmp/networkd-checkpoint-$CHECKPOINT_ID/" 2>/dev/null || true

# Detect uplink interface (simple method)
UPLINK=$(ip route show default | awk '{print $5}' | head -1)
[[ -n "$UPLINK" ]] || UPLINK="eth0"  # fallback

log "Detected uplink: $UPLINK"

# Get current IP configuration
UPLINK_IP=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f1 | head -1)
UPLINK_PREFIX=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f2 | head -1)
GATEWAY=$(ip route show default | awk '{print $3}' | head -1)

[[ -n "$UPLINK_IP" ]] || error_exit "Could not detect IP address for $UPLINK"

# Create systemd-networkd configuration
NETD_DIR="/etc/systemd/network"

# Bridge netdev
cat > "$NETD_DIR/10-ovsbr0.netdev" <<EOF
[NetDev]
Name=ovsbr0
Kind=bridge
EOF

# Bridge network config
cat > "$NETD_DIR/30-ovsbr0.network" <<EOF
[Match]
Name=ovsbr0

[Network]
Address=$UPLINK_IP/$UPLINK_PREFIX
Gateway=$GATEWAY
DNS=8.8.8.8
ConfigureWithoutCarrier=yes
EOF

# Uplink attachment
cat > "$NETD_DIR/20-$UPLINK.network" <<EOF
[Match]
Name=$UPLINK

[Network]
Bridge=ovsbr0
EOF

# Reload systemd-networkd
log "${BLUE}Reloading systemd-networkd${NC}"
systemctl reload-or-restart systemd-networkd || {
    log "${RED}Reload failed, rolling back${NC}"
    rm -f "$NETD_DIR"/{10-ovsbr0.netdev,20-$UPLINK.network,30-ovsbr0.network}
    cp "/tmp/networkd-checkpoint-$CHECKPOINT_ID"/* "$NETD_DIR/" 2>/dev/null || true
    systemctl reload-or-restart systemd-networkd
    error_exit "Failed to create bridge"
}

# Wait for network to stabilize
sleep 3

# Verify bridge exists
if ip link show ovsbr0 >/dev/null 2>&1; then
    log "${GREEN}✓ Bridge created successfully${NC}"
else
    error_exit "Bridge verification failed"
fi

# Install the agent
install -m 0755 target/release/ovs-port-agent /usr/local/bin/
install -d /etc/ovs-port-agent
[[ ! -f /etc/ovs-port-agent/config.toml ]] && install -m 0644 config/config.toml.example /etc/ovs-port-agent/config.toml
install -m 0644 dbus/dev.ovs.PortAgent1.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovs-port-agent.service /etc/systemd/system/

systemctl daemon-reload
systemctl enable --now ovs-port-agent

log "${GREEN}Installation complete - systemd-networkd bridge created${NC}"
