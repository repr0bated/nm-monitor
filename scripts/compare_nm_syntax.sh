#!/usr/bin/env bash
# Compare different NetworkManager syntaxes for OVS

echo "=== Testing NetworkManager OVS Syntax ==="
echo

echo "1. Testing documentation syntax (controller):"
echo "nmcli conn add type ovs-port conn.interface port0 controller bridge0"
nmcli conn add type ovs-port conn.interface port0 controller bridge0 2>&1 | head -5

echo
echo "2. Testing current syntax (connection.master):"
echo "nmcli conn add type ovs-port ifname port0 connection.master bridge0 connection.slave-type ovs-bridge"
nmcli conn add type ovs-port ifname port0 connection.master bridge0 connection.slave-type ovs-bridge --help 2>&1 | head -5

echo
echo "3. Checking nmcli version:"
nmcli --version

echo
echo "4. Checking valid properties for ovs-bridge:"
nmcli connection add type ovs-bridge help 2>&1 | grep -E "conn\.|connection\.|controller" | head -10