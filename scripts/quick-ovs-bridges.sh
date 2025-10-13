#!/usr/bin/env bash
set -euo pipefail

# QUICKEST path to working OVS bridges for Netmaker
# Just use ovs-vsctl, systemd-networkd will pick them up

echo "🚀 Quick OVS Bridge Creation"
echo "=============================="

# Check we're root
if [[ ${EUID} -ne 0 ]]; then
  echo "Must be run as root" >&2
  exit 1
fi

# Check OVS is available
if ! command -v ovs-vsctl >/dev/null 2>&1; then
  echo "Open vSwitch not installed" >&2
  exit 1
fi

# Parse arguments
UPLINK=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --uplink)
      UPLINK="$2"
      shift 2
      ;;
    --help|-h)
      cat <<HELP
Usage: $0 [--uplink IFACE]

Creates ovsbr0 and ovsbr1 using ovs-vsctl directly.
Minimal, fast, gets you working bridges NOW.

Options:
  --uplink IFACE    Physical interface to attach to ovsbr0 (optional)
HELP
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

echo "Creating ovsbr0..."
if ovs-vsctl br-exists ovsbr0 2>/dev/null; then
  echo "  ovsbr0 exists, deleting first..."
  ovs-vsctl del-br ovsbr0
fi

ovs-vsctl add-br ovsbr0 -- set bridge ovsbr0 datapath_type=netdev
ovs-vsctl set bridge ovsbr0 stp_enable=false
ovs-vsctl set bridge ovsbr0 mcast_snooping_enable=true

# Bring it up and get DHCP
ip link set ovsbr0 up

# Create minimal systemd-networkd config for DHCP
mkdir -p /etc/systemd/network
cat > /etc/systemd/network/10-ovsbr0.network <<EOF
[Match]
Name=ovsbr0

[Network]
DHCP=yes
EOF

echo "  ✓ ovsbr0 created"

# Attach uplink if specified
if [[ -n "${UPLINK}" ]]; then
  echo "Attaching ${UPLINK} to ovsbr0..."
  
  # Remove from any existing bridge
  if ovs-vsctl port-to-br "${UPLINK}" 2>/dev/null; then
    ovs-vsctl del-port "${UPLINK}"
  fi
  
  # Add to ovsbr0
  ovs-vsctl add-port ovsbr0 "${UPLINK}"
  
  # Remove IP from uplink (bridge gets the IP)
  ip addr flush dev "${UPLINK}" 2>/dev/null || true
  ip link set "${UPLINK}" up
  
  echo "  ✓ ${UPLINK} attached"
fi

echo "Creating ovsbr1..."
if ovs-vsctl br-exists ovsbr1 2>/dev/null; then
  echo "  ovsbr1 exists, deleting first..."
  ovs-vsctl del-br ovsbr1
fi

ovs-vsctl add-br ovsbr1 -- set bridge ovsbr1 datapath_type=netdev
ovs-vsctl set bridge ovsbr1 stp_enable=false
ovs-vsctl set bridge ovsbr1 mcast_snooping_enable=true

# Bring it up
ip link set ovsbr1 up

# Create minimal systemd-networkd config for DHCP
cat > /etc/systemd/network/11-ovsbr1.network <<EOF
[Match]
Name=ovsbr1

[Network]
DHCP=yes
EOF

echo "  ✓ ovsbr1 created"

# Restart systemd-networkd to pick up configs and start DHCP
echo "Restarting systemd-networkd..."
systemctl restart systemd-networkd

# Wait a moment for DHCP
echo "Waiting for DHCP..."
sleep 3

# Show status
echo ""
echo "✅ Bridges created!"
echo ""
echo "OVS Status:"
ovs-vsctl show
echo ""
echo "Bridge IPs:"
ip addr show ovsbr0 | grep -E "inet " || echo "  ovsbr0: No IP yet (DHCP may still be acquiring)"
ip addr show ovsbr1 | grep -E "inet " || echo "  ovsbr1: No IP yet (DHCP may still be acquiring)"
echo ""
echo "🎉 Ready for Netmaker!"

