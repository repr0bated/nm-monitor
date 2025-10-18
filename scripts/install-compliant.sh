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

# Check for non-interactive mode
NON_INTERACTIVE=${NON_INTERACTIVE:-false}
if [[ "$1" == "--help" ]] || [[ "$1" == "-h" ]]; then
    cat <<EOF
OVS Port Agent Installation Script

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --non-interactive, -y    Run in non-interactive mode (auto-use defaults)
    --help, -h               Show this help message

DESCRIPTION:
    Installs the OVS Port Agent with systemd-networkd integration.
    In non-interactive mode, automatically uses detected network defaults.

EXAMPLES:
    $0                      Interactive installation
    $0 --non-interactive    Non-interactive installation with defaults
EOF
    exit 0
fi

if [[ "$1" == "--non-interactive" ]] || [[ "$1" == "-y" ]]; then
    NON_INTERACTIVE=true
    log "${BLUE}Running in non-interactive mode for development/testing${NC}"
fi

rollback() {
    log "${RED}Rolling back${NC}"

    # Try to delete bridge using installed binary first, then local build
    /usr/local/bin/ovs-port-agent delete-bridge ovsbr0 2>/dev/null || \
    ./target/release/ovs-port-agent delete-bridge ovsbr0 2>/dev/null || true

    # Remove network configuration files
    rm -f "$NETD_DIR"/{10-ovsbr0-bridge.network,20-$UPLINK.network,30-ovsbr0.network}

    # Restore original network configuration
    if [[ -d "/tmp/checkpoint-$CHECKPOINT_ID" ]]; then
        cp -r "/tmp/checkpoint-$CHECKPOINT_ID"/* "$NETD_DIR/" 2>/dev/null || true
    fi

    # Reload networkd to apply restored configuration
    dbus-send --system --dest=org.freedesktop.systemd1 \
      /org/freedesktop/systemd1 \
      org.freedesktop.systemd1.Manager.ReloadOrRestartUnit \
      string:systemd-networkd.service string:replace 2>/dev/null || true

    # Clean up checkpoint directory
    rm -rf "/tmp/checkpoint-$CHECKPOINT_ID" 2>/dev/null || true
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

# Use detected default interface (non-interactive for dev/testing)
log "\n${YELLOW}Detected default interface: ${DEFAULT_IFACE:-none}${NC}"
if [[ "$NON_INTERACTIVE" == "true" ]]; then
    log "${GREEN}Using default interface automatically (non-interactive mode)${NC}"
    UPLINK=${DEFAULT_IFACE}
else
    read -p "Enter uplink interface name [${DEFAULT_IFACE}]: " UPLINK
    UPLINK=${UPLINK:-$DEFAULT_IFACE}
fi

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

if [[ "$NON_INTERACTIVE" == "true" ]]; then
    log "${GREEN}Proceeding with installation automatically (non-interactive mode)${NC}"
else
    read -p "Proceed with installation? [y/N]: " CONFIRM
    [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]] && error_exit "Installation cancelled"
fi

log "\n${BLUE}=== Starting Atomic Installation ===${NC}"

# Step 1: Create bridge via OVSDB wrapper with btrfs snapshots
log "${BLUE}[1/6] Creating OVS bridge via btrfs snapshot${NC}"
/usr/local/bin/ovsdb-dbus-wrapper create-bridge ovsbr0 || error_exit "Failed to create bridge"

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
log "${BLUE}[4/6] Adding $UPLINK to bridge via D-Bus btrfs snapshot${NC}"
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
[[ ! -f /etc/ovs-port-agent/config.json ]] && install -m 0644 config/config.json.example /etc/ovs-port-agent/config.json
install -m 0644 dbus/dev.ovs.PortAgent1.conf /etc/dbus-1/system.d/
install -m 0644 systemd/ovs-port-agent.service /etc/systemd/system/

# Configure JSON config file for ovsbr0 bridge
sed -i 's/"name": "vmbr0"/"name": "ovsbr0"/' /etc/ovs-port-agent/config.json

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

# Verify service started successfully
sleep 2
if systemctl is-active --quiet ovs-port-agent; then
    log "${GREEN}✓ ovs-port-agent service started successfully${NC}"
else
    log "${YELLOW}⚠ ovs-port-agent service may not have started properly${NC}"
    log "${YELLOW}Check status with: systemctl status ovs-port-agent${NC}"
fi

# Clean up checkpoint directory
rm -rf "/tmp/checkpoint-$CHECKPOINT_ID" 2>/dev/null || true

log "\n${GREEN}✓ Installation complete!${NC}"
log "\nBridge: ovsbr0"
log "Uplink: $UPLINK"
log "IP: $UPLINK_IP/$UPLINK_PREFIX"
log "Gateway: $GATEWAY"
