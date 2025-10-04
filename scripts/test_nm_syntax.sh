#!/usr/bin/env bash
# Test which NetworkManager syntax works

echo "Testing NetworkManager OVS syntax..."
echo

# Test 1: Documentation syntax
echo "Test 1: Using documentation syntax (conn.interface, controller)"
if nmcli conn add type ovs-bridge conn.interface testbr0 2>/dev/null; then
    echo "✓ Documentation syntax works!"
    nmcli conn delete ovs-bridge-testbr0 2>/dev/null || nmcli conn delete testbr0 2>/dev/null
else
    echo "✗ Documentation syntax failed"
fi

echo

# Test 2: Modern syntax
echo "Test 2: Using modern syntax (ifname, connection.master)"
if nmcli conn add type ovs-bridge con-name testbr1 ifname testbr1 2>/dev/null; then
    echo "✓ Modern syntax works!"
    nmcli conn delete testbr1 2>/dev/null
else
    echo "✗ Modern syntax failed"
fi

echo
echo "Checking nmcli help for correct syntax:"
nmcli connection add type ovs-bridge --help 2>&1 | grep -E "interface|master|controller" | head -10