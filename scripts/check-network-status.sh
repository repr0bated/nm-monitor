#!/usr/bin/env bash
# Quick network status check for systemd-networkd + OVS

echo "🔍 Network Status Check"
echo "======================="
echo ""

# Check systemd-networkd
echo "systemd-networkd:"
systemctl status systemd-networkd --no-pager | head -3
echo ""

# Check available interfaces
echo "Available network interfaces:"
networkctl list || ip link show
echo ""

# Check OVS bridges
if command -v ovs-vsctl >/dev/null 2>&1; then
  echo "OVS Bridges:"
  ovs-vsctl list-br 2>/dev/null || echo "  No bridges found"
  echo ""
  
  if ovs-vsctl br-exists ovsbr0 2>/dev/null; then
    echo "ovsbr0 status:"
    ovs-vsctl show | grep -A5 "Bridge ovsbr0" || true
    ip addr show ovsbr0 2>/dev/null || echo "  ovsbr0 not ready"
  fi
  echo ""
  
  if ovs-vsctl br-exists ovsbr1 2>/dev/null; then
    echo "ovsbr1 status:"
    ovs-vsctl show | grep -A5 "Bridge ovsbr1" || true
    ip addr show ovsbr1 2>/dev/null || echo "  ovsbr1 not ready"
  fi
else
  echo "OVS not installed"
fi

echo ""
echo "systemd-networkd config files:"
ls -lh /etc/systemd/network/*.net* 2>/dev/null || echo "  No .network/.netdev files found"

