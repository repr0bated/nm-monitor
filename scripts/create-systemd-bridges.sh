#!/usr/bin/env bash
set -euo pipefail

# Simple, reliable systemd-networkd OVS bridge creation
# No NetworkManager - pure systemd-networkd + OVS

echo "🌉 Creating OVS bridges with systemd-networkd"
echo "=============================================="

# Check we're root
if [[ ${EUID} -ne 0 ]]; then
  echo "Must be run as root" >&2
  exit 1
fi

# Check systemd-networkd is available
if ! command -v networkctl >/dev/null 2>&1; then
  echo "systemd-networkd not available" >&2
  exit 1
fi

# Check OVS is available
if ! command -v ovs-vsctl >/dev/null 2>&1; then
  echo "Open vSwitch not installed" >&2
  exit 1
fi

# Parse arguments
UPLINK=""
CREATE_OVSBR1=1  # Default to creating both bridges

while [[ $# -gt 0 ]]; do
  case "$1" in
    --uplink)
      UPLINK="$2"
      shift 2
      ;;
    --no-ovsbr1)
      CREATE_OVSBR1=0
      shift
      ;;
    --help|-h)
      cat <<HELP
Usage: $0 [options]

Options:
  --uplink IFACE    Physical interface to attach to ovsbr0 (optional)
  --no-ovsbr1       Don't create ovsbr1 (default: create both)
  --help            Show this help

Creates:
  ovsbr0 - Primary bridge with DHCP (optional uplink)
  ovsbr1 - Secondary bridge with DHCP (no uplink)

Both bridges are Netmaker-compatible with DHCP enabled.
HELP
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

# ============================================================================
# Create ovsbr0
# ============================================================================

echo "Creating ovsbr0 bridge..."

# 1. Create OVS bridge in the datapath
if ovs-vsctl br-exists ovsbr0 2>/dev/null; then
  echo "  ovsbr0 already exists, recreating..."
  ovs-vsctl del-br ovsbr0
fi

ovs-vsctl add-br ovsbr0
ovs-vsctl set bridge ovsbr0 datapath_type=netdev
ovs-vsctl set bridge ovsbr0 stp_enable=false
ovs-vsctl set bridge ovsbr0 mcast_snooping_enable=true

# 2. Create systemd-networkd .netdev file for ovsbr0
cat > /etc/systemd/network/10-ovsbr0.netdev <<EOF
[NetDev]
Name=ovsbr0
Kind=openvswitch
EOF

# 3. Create systemd-networkd .network file for ovsbr0
cat > /etc/systemd/network/10-ovsbr0.network <<EOF
[Match]
Name=ovsbr0

[Network]
DHCP=yes
IPv6AcceptRA=yes
LinkLocalAddressing=yes

[DHCP]
UseDNS=yes
UseRoutes=yes
EOF

echo "  ✓ ovsbr0 created"

# 4. Attach uplink if specified
if [[ -n "${UPLINK}" ]]; then
  echo "Attaching uplink ${UPLINK} to ovsbr0..."
  
  # Add port to OVS bridge
  if ovs-vsctl port-to-br "${UPLINK}" 2>/dev/null; then
    echo "  ${UPLINK} already attached to a bridge, removing..."
    ovs-vsctl del-port "${UPLINK}"
  fi
  
  ovs-vsctl add-port ovsbr0 "${UPLINK}"
  
  # Create .network file to manage uplink with systemd-networkd
  cat > "/etc/systemd/network/20-${UPLINK}.network" <<EOF
[Match]
Name=${UPLINK}

[Network]
# No IP on enslaved interface
LinkLocalAddressing=no
LLDP=yes
EmitLLDP=yes
EOF

  echo "  ✓ ${UPLINK} attached to ovsbr0"
fi

# ============================================================================
# Create ovsbr1 (if requested)
# ============================================================================

if [[ ${CREATE_OVSBR1} -eq 1 ]]; then
  echo "Creating ovsbr1 bridge..."
  
  # 1. Create OVS bridge
  if ovs-vsctl br-exists ovsbr1 2>/dev/null; then
    echo "  ovsbr1 already exists, recreating..."
    ovs-vsctl del-br ovsbr1
  fi
  
  ovs-vsctl add-br ovsbr1
  ovs-vsctl set bridge ovsbr1 datapath_type=netdev
  ovs-vsctl set bridge ovsbr1 stp_enable=false
  ovs-vsctl set bridge ovsbr1 mcast_snooping_enable=true
  
  # 2. Create .netdev file for ovsbr1
  cat > /etc/systemd/network/11-ovsbr1.netdev <<EOF
[NetDev]
Name=ovsbr1
Kind=openvswitch
EOF
  
  # 3. Create .network file for ovsbr1
  cat > /etc/systemd/network/11-ovsbr1.network <<EOF
[Match]
Name=ovsbr1

[Network]
DHCP=yes
IPv6AcceptRA=yes
LinkLocalAddressing=yes

[DHCP]
UseDNS=yes
UseRoutes=yes
EOF
  
  echo "  ✓ ovsbr1 created"
fi

# ============================================================================
# Apply configuration
# ============================================================================

echo "Applying systemd-networkd configuration..."

# Restart systemd-networkd to pick up new bridges
systemctl restart systemd-networkd

# Wait for bridges to come up
sleep 3

# Verify bridges exist
echo ""
echo "Verifying bridge status..."
networkctl status ovsbr0 || echo "  ⚠️  ovsbr0 not ready yet"

if [[ ${CREATE_OVSBR1} -eq 1 ]]; then
  networkctl status ovsbr1 || echo "  ⚠️  ovsbr1 not ready yet"
fi

# Show OVS status
echo ""
echo "OVS Bridge Status:"
ovs-vsctl show

echo ""
echo "✅ Bridge creation complete!"
echo ""
echo "Created bridges:"
echo "  - ovsbr0 (DHCP enabled)"
[[ ${CREATE_OVSBR1} -eq 1 ]] && echo "  - ovsbr1 (DHCP enabled)"
[[ -n "${UPLINK}" ]] && echo "  - Uplink: ${UPLINK} → ovsbr0"
echo ""
echo "Verify with:"
echo "  networkctl list"
echo "  ip addr show ovsbr0"
[[ ${CREATE_OVSBR1} -eq 1 ]] && echo "  ip addr show ovsbr1"
echo ""
echo "These bridges are now ready for Netmaker!"

