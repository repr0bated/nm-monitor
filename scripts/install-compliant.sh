#!/usr/bin/env bash
# Pure D-Bus install - introspect network and prompt for uplink

set -euo pipefail

readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly BLUE='\033[0;34m'
readonly YELLOW='\033[1;33m'
readonly NC='\033[0m'

log() { echo -e "$1" >&2; }
error_exit() { log "${RED}[ERROR]${NC} $1"; exit 1; }

rollback() {
    log "${RED}Rolling back${NC}"
    ./target/release/ovs-port-agent delete-bridge ovsbr0 2>/dev/null || true
    rm -f "$NETD_DIR"/{10-ovsbr0-bridge.network,20-$UPLINK.network,30-ovsbr0.network}
    cp "/tmp/checkpoint-$CHECKPOINT_ID"/* "$NETD_DIR/" 2>/dev/null || true
    dbus-send --system --dest=org.freedesktop.systemd1 \
      /org/freedesktop/systemd1 \
      org.freedesktop.systemd1.Manager.ReloadOrRestartUnit \
      string:systemd-networkd.service string:replace 2>/dev/null || true
}

[[ $EUID -eq 0 ]] || error_exit "Must run as root"

# Ensure ovsdb-server is running
systemctl is-active --quiet ovsdb-server || systemctl start ovsdb-server

cd "$(dirname "$0")/.."
cargo build --release || error_exit "Failed to build"

# Install and start OVSDB D-Bus wrapper
log "${BLUE}Installing OVSDB D-Bus wrapper${NC}"
systemctl stop ovsdb-dbus-wrapper 2>/dev/null || true
install -m 0755 target/release/ovsdb-dbus-wrapper /usr/local/bin/
install -m 0644 dbus/org.openvswitch.ovsdb.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovsdb-dbus-wrapper.service /etc/systemd/system/

systemctl daemon-reload
systemctl enable --now ovsdb-dbus-wrapper
sleep 2  # Wait for D-Bus service to register

log "${GREEN}✓ OVSDB D-Bus wrapper installed and running${NC}"

# Install and start OVSDB D-Bus wrapper
log "${BLUE}Installing OVSDB D-Bus wrapper${NC}"
install -m 0755 target/release/ovsdb-dbus-wrapper /usr/local/bin/
install -m 0644 dbus/org.openvswitch.ovsdb.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovsdb-dbus-wrapper.service /etc/systemd/system/

systemctl daemon-reload
systemctl enable --now ovsdb-dbus-wrapper
sleep 2  # Wait for D-Bus service to register

# Backup
CHECKPOINT_ID=$(date +%s)
NETD_DIR="/etc/systemd/network"
mkdir -p "/tmp/checkpoint-$CHECKPOINT_ID"
cp -r "$NETD_DIR"/* "/tmp/checkpoint-$CHECKPOINT_ID/" 2>/dev/null || true

log "${BLUE}=== Network Introspection ===${NC}"

# Introspect available interfaces
log "${YELLOW}Available network interfaces:${NC}"
ip -o link show | awk -F': ' '{print $2}' | grep -v '^lo$' | nl

# Introspect current routes
log "\n${YELLOW}Current routing table:${NC}"
ip route show

# Detect default route interface
DEFAULT_IFACE=$(ip route show default | awk '{print $5}' | head -1)

# Prompt for uplink interface
log "\n${YELLOW}Detected default interface: ${DEFAULT_IFACE:-none}${NC}"
read -p "Enter uplink interface name [${DEFAULT_IFACE}]: " UPLINK
UPLINK=${UPLINK:-$DEFAULT_IFACE}

[[ -z "$UPLINK" ]] && error_exit "No uplink interface specified"
[[ ! -d "/sys/class/net/$UPLINK" ]] && error_exit "Interface $UPLINK does not exist"

log "${GREEN}Using uplink: $UPLINK${NC}"

# Introspect IP configuration
log "\n${BLUE}=== IP Configuration Introspection ===${NC}"

UPLINK_IP=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f1 | head -1)
UPLINK_PREFIX=$(ip -o -4 addr show "$UPLINK" | awk '{print $4}' | cut -d/ -f2 | head -1)
GATEWAY=$(ip route show default | awk '{print $3}' | head -1)

log "IP Address: ${UPLINK_IP}/${UPLINK_PREFIX}"
log "Gateway: ${GATEWAY}"

[[ -z "$UPLINK_IP" ]] && error_exit "No IP address found on $UPLINK"
[[ -z "$GATEWAY" ]] && error_exit "No default gateway found"

# Introspect DNS
DNS_SERVERS=$(grep '^nameserver' /etc/resolv.conf | awk '{print $2}' | head -2 | tr '\n' ' ')
log "DNS Servers: ${DNS_SERVERS:-8.8.8.8}"

# Confirm configuration
log "\n${YELLOW}=== Configuration Summary ===${NC}"
log "Uplink Interface: $UPLINK"
log "IP Address: $UPLINK_IP/$UPLINK_PREFIX"
log "Gateway: $GATEWAY"
log "DNS: ${DNS_SERVERS:-8.8.8.8}"
log "Bridge: ovsbr0"

read -p "Proceed with installation? [y/N]: " CONFIRM
[[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]] && error_exit "Installation cancelled"

log "\n${BLUE}=== Starting Atomic Installation ===${NC}"

# Step 1: Create bridge via OVSDB D-Bus
log "${BLUE}[1/6] Creating OVS bridge via OVSDB D-Bus${NC}"
./target/release/ovs-port-agent create-bridge ovsbr0 || error_exit "Failed to create bridge"

# Step 2: Create systemd-networkd configs BEFORE adding port
log "${BLUE}[2/6] Creating systemd-networkd configuration${NC}"

cat > "$NETD_DIR/30-ovsbr0.network" <<EOF
[Match]
Name=ovsbr0

[Network]
Address=$UPLINK_IP/$UPLINK_PREFIX
Gateway=$GATEWAY
DNS=${DNS_SERVERS:-8.8.8.8}
ConfigureWithoutCarrier=yes

[Route]
Gateway=$GATEWAY
Metric=100
EOF

cat > "$NETD_DIR/20-$UPLINK.network" <<EOF
[Match]
Name=$UPLINK

[Network]
ConfigureWithoutCarrier=yes

[Link]
RequiredForOnline=no
EOF

# Step 3: Reload networkd BEFORE adding port (so it knows about bridge)
log "${BLUE}[3/6] Reloading systemd-networkd via D-Bus${NC}"
dbus-send --system --dest=org.freedesktop.systemd1 \
  /org/freedesktop/systemd1 \
  org.freedesktop.systemd1.Manager.ReloadOrRestartUnit \
  string:systemd-networkd.service string:replace || {
    rollback
    error_exit "Failed to reload networkd"
}

sleep 2

# Step 4: NOW add port (networkd won't interfere)
log "${BLUE}[4/6] Adding $UPLINK to bridge via OVSDB D-Bus${NC}"
./target/release/ovs-port-agent add-port ovsbr0 "$UPLINK" || {
    rollback
    error_exit "Failed to add port"
}

# Step 5: Bring bridge up and wait for IP
log "${BLUE}[5/6] Waiting for bridge to get IP${NC}"
sleep 5

# Step 6: Test connectivity
log "${BLUE}[6/6] Testing connectivity${NC}"
if ! ping -c 3 -W 5 8.8.8.8 >/dev/null 2>&1; then
    rollback
    error_exit "Connectivity test failed"
fi

log "${GREEN}✓ Atomic handover complete${NC}"

# Install agent
log "\n${BLUE}=== Installing Agent ===${NC}"
install -m 0755 target/release/ovs-port-agent /usr/local/bin/
install -d /etc/ovs-port-agent
[[ ! -f /etc/ovs-port-agent/config.toml ]] && install -m 0644 config/config.toml.example /etc/ovs-port-agent/config.toml
install -m 0644 dbus/dev.ovs.PortAgent1.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovs-port-agent.service /etc/systemd/system/

sed -i 's/bridge_name = .*/bridge_name = "ovsbr0"/' /etc/ovs-port-agent/config.toml

# Enable and start via D-Bus
dbus-send --system --dest=org.freedesktop.systemd1 \
  /org/freedesktop/systemd1 \
  org.freedesktop.systemd1.Manager.Reload

dbus-send --system --dest=org.freedesktop.systemd1 \
  /org/freedesktop/systemd1 \
  org.freedesktop.systemd1.Manager.EnableUnitFiles \
  array:string:ovs-port-agent.service boolean:false boolean:true

dbus-send --system --dest=org.freedesktop.systemd1 \
  /org/freedesktop/systemd1 \
  org.freedesktop.systemd1.Manager.StartUnit \
  string:ovs-port-agent.service string:replace

log "\n${GREEN}✓ Installation complete!${NC}"
log "\nBridge: ovsbr0"
log "Uplink: $UPLINK"
log "IP: $UPLINK_IP/$UPLINK_PREFIX"
log "Gateway: $GATEWAY"
