#!/bin/bash
# Review logs from failed OVS configuration attempts

echo "🔍 REVIEWING FAILURE LOGS..."
echo "============================"

echo ""
echo "📋 OVS-PORT-AGENT SERVICE LOGS:"
sudo journalctl -u ovs-port-agent --since "1 hour ago" -n 20

echo ""
echo "📋 SYSTEMD-NETWORKD LOGS:"
sudo journalctl -u systemd-networkd --since "1 hour ago" -n 20

echo ""
echo "📋 SYSTEM LOGS (network related):"
sudo journalctl --since "1 hour ago" | grep -i "network\|ovs\|bridge\|connect" | tail -20

echo ""
echo "📋 OVS STATUS:"
sudo ovs-vsctl show 2>/dev/null || echo "OVS not available"

echo ""
echo "📋 NETWORK INTERFACES:"
ip link show

echo ""
echo "📋 SYSTEMD-NETWORKD FILES:"
ls -la /etc/systemd/network/

echo ""
echo "📋 LEDGER LOGS:"
sudo cat /var/lib/ovs-port-agent/ledger.jsonl 2>/dev/null | tail -10 || echo "No ledger found"

echo ""
echo "🎯 LOG ANALYSIS:"
echo "  • Look for 'ovs-interface' errors"
echo "  • Look for 'plugins' field errors"  
echo "  • Look for network config application failures"
echo "  • Look for connectivity loss events"
