#!/usr/bin/env bash
# Introspect current network configuration and generate YAML config

set -euo pipefail

echo "=========================================="
echo " Network Introspection"
echo "=========================================="
echo ""

# Find primary interface (first non-lo, non-ovs, non-docker with IP)
# Prefer physical/wifi interfaces over bridges
PRIMARY_IFACE=$(ip -o -4 addr show | grep -v "lo" | grep -v "ovsbr" | grep -v "docker" | grep -v "br-" | grep -v "veth" | head -1 | awk '{print $2}')

# Fallback to any interface with IP if no physical found
if [[ -z "${PRIMARY_IFACE}" ]]; then
  echo "No physical interface found, checking all interfaces..."
  PRIMARY_IFACE=$(ip -o -4 addr show | grep -v "^1:" | grep -v "lo" | grep -v "docker" | grep -v "br-" | grep -v "veth" | head -1 | awk '{print $2}')
fi

if [[ -z "${PRIMARY_IFACE}" ]]; then
  echo ""
  echo "ERROR: Could not detect primary interface" >&2
  echo ""
  echo "Available interfaces:"
  ip -brief addr show | grep -v "lo"
  echo ""
  echo "This might mean:"
  echo "  1. Network already configured with OVS bridges"
  echo "  2. No active network interface found"
  echo ""
  echo "Solutions:"
  echo "  - Use existing config: --network-config FILE"
  echo "  - Manually create config based on: ip addr show"
  exit 1
fi

echo "Detected Primary Interface: ${PRIMARY_IFACE}"

# Get IP configuration
IP_ADDR=$(ip -o -4 addr show "${PRIMARY_IFACE}" | awk '{print $4}' | cut -d/ -f1)
PREFIX=$(ip -o -4 addr show "${PRIMARY_IFACE}" | awk '{print $4}' | cut -d/ -f2)
GATEWAY=$(ip route show default | grep "${PRIMARY_IFACE}" | awk '{print $3}' | head -1)

echo "  IP Address: ${IP_ADDR}/${PREFIX}"
echo "  Gateway: ${GATEWAY:-none}"

# Get DNS servers
DNS_SERVERS=()
if [[ -f /etc/resolv.conf ]]; then
  while IFS= read -r line; do
    if [[ $line =~ ^nameserver[[:space:]]+([0-9.]+) ]]; then
      DNS_SERVERS+=("${BASH_REMATCH[1]}")
    fi
  done < /etc/resolv.conf
fi

if [[ ${#DNS_SERVERS[@]} -gt 0 ]]; then
  echo "  DNS: ${DNS_SERVERS[*]}"
else
  echo "  DNS: none detected"
fi

# Check if DHCP
IS_DHCP=false
if command -v networkctl >/dev/null 2>&1; then
  if networkctl status "${PRIMARY_IFACE}" 2>/dev/null | grep -qi "DHCP.*yes"; then
    IS_DHCP=true
  fi
fi

echo "  Using DHCP: ${IS_DHCP}"
echo ""

# Capture ALL hardware properties
MAC_ADDRESS=$(ip link show "${PRIMARY_IFACE}" | grep -oP 'link/ether \K[^ ]+' | head -1)
MTU=$(ip link show "${PRIMARY_IFACE}" | grep -oP 'mtu \K[0-9]+')
OPERSTATE=$(cat "/sys/class/net/${PRIMARY_IFACE}/operstate" 2>/dev/null || echo "unknown")
SPEED=$(cat "/sys/class/net/${PRIMARY_IFACE}/speed" 2>/dev/null || echo "unknown")
DUPLEX=$(cat "/sys/class/net/${PRIMARY_IFACE}/duplex" 2>/dev/null || echo "unknown")
TXQUEUELEN=$(ip link show "${PRIMARY_IFACE}" | grep -oP 'qlen \K[0-9]+' || echo "1000")

echo "Hardware Properties:"
echo "  MAC: ${MAC_ADDRESS}"
echo "  MTU: ${MTU}"
echo "  Speed: ${SPEED}"
echo "  Duplex: ${DUPLEX}"
echo ""

# Determine bridge name (can be overridden by argument)
BRIDGE_NAME="${2:-vmbr0}"
echo "Target Bridge: ${BRIDGE_NAME}"
echo ""

# Generate configuration
OUTPUT_FILE="${1:-/tmp/network-introspected-$(date +%Y%m%d_%H%M%S).yaml}"

cat > "${OUTPUT_FILE}" <<EOF
# Auto-generated network configuration
# Created: $(date)
# Source: ${PRIMARY_IFACE} (${IP_ADDR}/${PREFIX})

version: "1.0"

plugins:
  net:
    interfaces:
      # OVS bridge with uplink from ${PRIMARY_IFACE}
      - name: ${BRIDGE_NAME}
        type: ovs-bridge
        ports:
          - ${PRIMARY_IFACE}
        ipv4:
          enabled: true
EOF

if [[ "${IS_DHCP}" == "true" ]]; then
  cat >> "${OUTPUT_FILE}" <<EOF
          dhcp: true  # Detected DHCP on ${PRIMARY_IFACE}
EOF
else
  cat >> "${OUTPUT_FILE}" <<EOF
          dhcp: false
          address:
            - ip: ${IP_ADDR}
              prefix: ${PREFIX}
EOF
  
  if [[ -n "${GATEWAY}" ]]; then
    cat >> "${OUTPUT_FILE}" <<EOF
          gateway: ${GATEWAY}
EOF
  fi
  
  if [[ ${#DNS_SERVERS[@]} -gt 0 ]]; then
    cat >> "${OUTPUT_FILE}" <<EOF
          dns:
EOF
    for dns in "${DNS_SERVERS[@]}"; do
      cat >> "${OUTPUT_FILE}" <<EOF
            - ${dns}
EOF
    done
  fi
fi

cat >> "${OUTPUT_FILE}" <<EOF
      
      # Physical uplink (enslaved to bridge)
      - name: ${PRIMARY_IFACE}
        type: ethernet
        controller: ${BRIDGE_NAME}
        ipv4:
          enabled: false  # IP moves to bridge
EOF

echo "=========================================="
echo "Generated Configuration"
echo "=========================================="
echo ""
cat "${OUTPUT_FILE}"
echo ""
echo "=========================================="
echo ""
echo "Configuration saved to: ${OUTPUT_FILE}"
echo ""
echo "To install:"
echo "  sudo ./scripts/install-with-network-plugin.sh \\"
echo "    --network-config ${OUTPUT_FILE} \\"
echo "    --system"
echo ""
echo "To add ovsbr1 for Docker:"
echo "  sudo ./scripts/install-with-network-plugin.sh \\"
echo "    --network-config ${OUTPUT_FILE} \\"
echo "    --with-ovsbr1 \\"
echo "    --system"
echo ""

