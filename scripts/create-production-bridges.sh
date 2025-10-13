#!/usr/bin/env bash
set -euo pipefail

# Production OVS Bridge Creation
# - ovsbr0: Static IP with uplink (for VPS)
# - ovsbr1: Docker-integrated bridge

echo "🌉 Production OVS Bridge Setup"
echo "==============================="

# Check we're root
if [[ ${EUID} -ne 0 ]]; then
  echo "Must be run as root" >&2
  exit 1
fi

# Check OVS is available
if ! command -v ovs-vsctl >/dev/null 2>&1; then
  echo "❌ Open vSwitch not installed!" >&2
  echo "Install with: sudo apt-get install -y openvswitch-switch" >&2
  exit 1
fi

# Parse arguments
UPLINK=""
IP_ADDRESS=""
GATEWAY=""
DNS="1.1.1.1,8.8.8.8"

show_usage() {
  cat <<HELP
Usage: $0 [options]

This script creates two OVS bridges:
  ovsbr0 - Primary bridge with static IP and uplink
  ovsbr1 - Docker-integrated bridge

Options:
  --uplink IFACE         Physical interface (e.g., eth0, enp2s0)
  --ip IP/CIDR           Static IP with CIDR (e.g., 192.168.1.10/24)
  --gateway IP           Gateway IP (e.g., 192.168.1.1)
  --dns IP[,IP]          DNS servers (default: 1.1.1.1,8.8.8.8)
  --help                 Show this help

Example:
  $0 --uplink eth0 --ip 192.168.1.10/24 --gateway 192.168.1.1

Interactive mode (no args):
  $0
HELP
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --uplink)
      UPLINK="$2"
      shift 2
      ;;
    --ip)
      IP_ADDRESS="$2"
      shift 2
      ;;
    --gateway)
      GATEWAY="$2"
      shift 2
      ;;
    --dns)
      DNS="$2"
      shift 2
      ;;
    --help|-h)
      show_usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      show_usage
      exit 1
      ;;
  esac
done

# Interactive prompts if not provided
if [[ -z "${UPLINK}" ]]; then
  echo ""
  echo "Available network interfaces:"
  ip -br link show | grep -v "^lo" | awk '{print "  " $1 " - " $3}'
  echo ""
  read -p "Enter uplink interface name: " UPLINK
fi

if [[ -z "${IP_ADDRESS}" ]]; then
  echo ""
  echo "Current IP on ${UPLINK}:"
  ip addr show "${UPLINK}" | grep "inet " || echo "  No IP configured"
  echo ""
  read -p "Enter static IP with CIDR (e.g., 192.168.1.10/24): " IP_ADDRESS
fi

if [[ -z "${GATEWAY}" ]]; then
  echo ""
  read -p "Enter gateway IP (e.g., 192.168.1.1): " GATEWAY
fi

# Validate inputs
if ! ip link show "${UPLINK}" &>/dev/null; then
  echo "❌ Interface ${UPLINK} not found" >&2
  exit 1
fi

if ! echo "${IP_ADDRESS}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/[0-9]+$'; then
  echo "❌ Invalid IP address format: ${IP_ADDRESS}" >&2
  echo "   Expected format: IP/CIDR (e.g., 192.168.1.10/24)" >&2
  exit 1
fi

echo ""
echo "Configuration Summary:"
echo "  Uplink:  ${UPLINK}"
echo "  IP:      ${IP_ADDRESS}"
echo "  Gateway: ${GATEWAY}"
echo "  DNS:     ${DNS}"
echo ""
echo "⚠️  This will configure your network. Press Ctrl+C to abort..."
sleep 3

# ============================================================================
# Create ovsbr0 with static IP
# ============================================================================

echo ""
echo "Step 1: Creating ovsbr0 (primary bridge with static IP)..."

if ovs-vsctl br-exists ovsbr0 2>/dev/null; then
  echo "  Deleting existing ovsbr0..."
  ovs-vsctl del-br ovsbr0
fi

ovs-vsctl add-br ovsbr0 -- set bridge ovsbr0 datapath_type=netdev
ovs-vsctl set bridge ovsbr0 stp_enable=false
ovs-vsctl set bridge ovsbr0 mcast_snooping_enable=true

echo "  Adding ${UPLINK} to ovsbr0..."
if ovs-vsctl port-to-br "${UPLINK}" 2>/dev/null; then
  ovs-vsctl del-port "${UPLINK}"
fi
ovs-vsctl add-port ovsbr0 "${UPLINK}"

# Bring interfaces up
ip link set "${UPLINK}" up
ip link set ovsbr0 up

echo "  Configuring static IP on ovsbr0..."
# Remove any existing IPs
ip addr flush dev ovsbr0 2>/dev/null || true
ip addr flush dev "${UPLINK}" 2>/dev/null || true

# Add static IP to bridge
ip addr add "${IP_ADDRESS}" dev ovsbr0

# Add default route
ip route del default 2>/dev/null || true
ip route add default via "${GATEWAY}" dev ovsbr0

# Create systemd-networkd config for persistence
mkdir -p /etc/systemd/network

cat > /etc/systemd/network/10-ovsbr0.network <<EOF
[Match]
Name=ovsbr0

[Network]
Address=${IP_ADDRESS}
Gateway=${GATEWAY}
DNS=${DNS//,/ }
EOF

cat > "/etc/systemd/network/20-${UPLINK}.network" <<EOF
[Match]
Name=${UPLINK}

[Network]
# Enslaved to OVS bridge, no IP
LinkLocalAddressing=no
EOF

echo "  ✅ ovsbr0 configured with ${IP_ADDRESS}"

# ============================================================================
# Create ovsbr1 for Docker
# ============================================================================

echo ""
echo "Step 2: Creating ovsbr1 (Docker bridge)..."

if ovs-vsctl br-exists ovsbr1 2>/dev/null; then
  echo "  Deleting existing ovsbr1..."
  ovs-vsctl del-br ovsbr1
fi

ovs-vsctl add-br ovsbr1 -- set bridge ovsbr1 datapath_type=netdev
ovs-vsctl set bridge ovsbr1 stp_enable=false
ovs-vsctl set bridge ovsbr1 mcast_snooping_enable=true
ip link set ovsbr1 up

# Configure ovsbr1 with Docker-friendly subnet
DOCKER_SUBNET="172.18.0.1/16"
ip addr add "${DOCKER_SUBNET}" dev ovsbr1 2>/dev/null || true

cat > /etc/systemd/network/11-ovsbr1.network <<EOF
[Match]
Name=ovsbr1

[Network]
Address=${DOCKER_SUBNET}
IPForward=yes
IPMasquerade=yes
EOF

# Enable IP forwarding for Docker connectivity
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
echo "net.ipv4.ip_forward=1" > /etc/sysctl.d/90-ovs-bridges.conf

echo "  ✅ ovsbr1 configured for Docker (${DOCKER_SUBNET})"

# ============================================================================
# Docker Integration
# ============================================================================

if command -v docker >/dev/null 2>&1; then
  echo ""
  echo "Step 3: Configuring Docker to use ovsbr1..."
  
  mkdir -p /etc/docker
  
  # Check if daemon.json exists
  if [[ -f /etc/docker/daemon.json ]]; then
    echo "  ⚠️  /etc/docker/daemon.json already exists"
    echo "  Please manually add ovsbr1 configuration:"
    cat <<DOCKERJSON
{
  "bridge": "ovsbr1",
  "fixed-cidr": "172.18.1.0/24"
}
DOCKERJSON
  else
    cat > /etc/docker/daemon.json <<EOF
{
  "bridge": "ovsbr1",
  "fixed-cidr": "172.18.1.0/24"
}
EOF
    echo "  Created /etc/docker/daemon.json"
    
    echo "  Restarting Docker..."
    systemctl restart docker 2>/dev/null || true
  fi
  
  echo "  ✅ Docker configured for ovsbr1"
else
  echo ""
  echo "  ℹ️  Docker not installed - ovsbr1 ready for future Docker installation"
fi

# ============================================================================
# Apply and verify
# ============================================================================

echo ""
echo "Step 4: Applying configuration..."
systemctl restart systemd-networkd 2>/dev/null || true
sleep 2

echo ""
echo "========================================="
echo "✅ Bridge Configuration Complete!"
echo "========================================="
echo ""
echo "OVS Bridges:"
ovs-vsctl show
echo ""
echo "ovsbr0 (Primary with static IP):"
ip addr show ovsbr0 | grep -E "inet |state"
echo ""
echo "ovsbr1 (Docker bridge):"
ip addr show ovsbr1 | grep -E "inet |state"
echo ""
echo "Routes:"
ip route show | grep -E "default|ovsbr"
echo ""
echo "🎉 Ready for production!"
echo ""
echo "To connect Docker containers to ovsbr1:"
echo "  docker run --network bridge <image>"
echo ""
echo "To attach containers to OVS ports:"
echo "  ovs-docker add-port ovsbr1 eth0 <container_id> --ipaddress=172.18.1.10/24"

