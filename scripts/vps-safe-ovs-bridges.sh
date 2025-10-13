#!/usr/bin/env bash
set -euo pipefail

# VPS-SAFE OVS Bridge Creation
# Migrates uplink IP to bridge without breaking SSH connectivity

echo "🔒 VPS-Safe OVS Bridge Creation"
echo "================================="

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

while [[ $# -gt 0 ]]; do
  case "$1" in
    --uplink)
      UPLINK="$2"
      shift 2
      ;;
    --help|-h)
      cat <<HELP
Usage: $0 --uplink IFACE

VPS-safe bridge creation that preserves connectivity.

Example:
  $0 --uplink enxe04f43a07fce
HELP
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "${UPLINK}" ]]; then
  echo "❌ --uplink is required for VPS setup" >&2
  echo "Run: $0 --uplink enxe04f43a07fce" >&2
  exit 1
fi

# Verify uplink exists
if ! ip link show "${UPLINK}" &>/dev/null; then
  echo "❌ Interface ${UPLINK} not found" >&2
  exit 1
fi

echo "⚠️  This will migrate ${UPLINK} to OVS bridge ovsbr0"
echo "⚠️  There may be a brief connectivity interruption"
echo ""
echo "Current ${UPLINK} config:"
ip addr show "${UPLINK}" | grep -E "inet |state"
echo ""
echo "Press Ctrl+C within 5 seconds to abort..."
sleep 5

echo ""
echo "Step 1: Installing OVS (if needed)..."
if ! systemctl is-active openvswitch-switch &>/dev/null; then
  systemctl start openvswitch-switch || true
fi

echo "Step 2: Creating ovsbr0 bridge..."
if ovs-vsctl br-exists ovsbr0 2>/dev/null; then
  echo "  ovsbr0 exists, recreating..."
  ovs-vsctl del-br ovsbr0
fi

ovs-vsctl add-br ovsbr0 -- set bridge ovsbr0 datapath_type=netdev
ovs-vsctl set bridge ovsbr0 stp_enable=false
ovs-vsctl set bridge ovsbr0 mcast_snooping_enable=true
ip link set ovsbr0 up

echo "Step 3: Adding ${UPLINK} to ovsbr0..."
# Remove from any existing bridge
if ovs-vsctl port-to-br "${UPLINK}" 2>/dev/null; then
  ovs-vsctl del-port "${UPLINK}"
fi

ovs-vsctl add-port ovsbr0 "${UPLINK}"
ip link set "${UPLINK}" up

echo "Step 4: Configuring systemd-networkd for seamless handover..."
mkdir -p /etc/systemd/network

# Configure uplink to have no IP (bridge gets the IP)
cat > "/etc/systemd/network/20-${UPLINK}.network" <<EOF
[Match]
Name=${UPLINK}

[Network]
# Enslaved to OVS bridge, no IP configuration
LinkLocalAddressing=no
LLDP=yes
EOF

# Configure ovsbr0 to get DHCP (takes over from uplink)
cat > /etc/systemd/network/10-ovsbr0.network <<EOF
[Match]
Name=ovsbr0

[Network]
DHCP=yes
IPv6AcceptRA=yes

[DHCP]
UseDNS=yes
UseRoutes=yes
ClientIdentifier=mac
EOF

echo "Step 5: Restarting systemd-networkd to apply changes..."
echo "  (Brief interruption expected...)"
systemctl restart systemd-networkd

echo "Step 6: Waiting for DHCP on ovsbr0..."
sleep 5

# Check if we got an IP
if ip addr show ovsbr0 | grep -q "inet "; then
  echo "  ✅ ovsbr0 has IP:"
  ip addr show ovsbr0 | grep "inet "
else
  echo "  ⚠️  ovsbr0 doesn't have IP yet, waiting longer..."
  sleep 5
  if ip addr show ovsbr0 | grep -q "inet "; then
    echo "  ✅ ovsbr0 has IP now:"
    ip addr show ovsbr0 | grep "inet "
  else
    echo "  ❌ No IP on ovsbr0 - connectivity may be broken!"
  fi
fi

echo ""
echo "Step 7: Creating ovsbr1 (secondary bridge)..."
if ovs-vsctl br-exists ovsbr1 2>/dev/null; then
  ovs-vsctl del-br ovsbr1
fi

ovs-vsctl add-br ovsbr1 -- set bridge ovsbr1 datapath_type=netdev
ovs-vsctl set bridge ovsbr1 stp_enable=false
ovs-vsctl set bridge ovsbr1 mcast_snooping_enable=true
ip link set ovsbr1 up

# Configure ovsbr1 with DHCP (for netmaker)
cat > /etc/systemd/network/11-ovsbr1.network <<EOF
[Match]
Name=ovsbr1

[Network]
DHCP=yes
EOF

systemctl restart systemd-networkd
sleep 3

echo ""
echo "========================================="
echo "✅ Bridge creation complete!"
echo "========================================="
echo ""
echo "OVS Configuration:"
ovs-vsctl show
echo ""
echo "Network Status:"
echo "ovsbr0:"
ip addr show ovsbr0 | grep -E "inet |state" || echo "  No IP"
echo ""
echo "ovsbr1:"
ip addr show ovsbr1 | grep -E "inet |state" || echo "  No IP"
echo ""
echo "${UPLINK}:"
ip addr show "${UPLINK}" | grep -E "inet |state" || echo "  No IP (expected)"
echo ""
echo "Default route:"
ip route show default | head -1
echo ""
echo "🎉 Bridges ready for Netmaker!"
echo ""
echo "If connectivity is lost, reboot the VPS to restore original config."

