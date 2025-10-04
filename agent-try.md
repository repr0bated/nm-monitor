# NetworkManager OVS Bridge Debug Session

## Overview

This document captures the debugging session for creating OVS bridges with NetworkManager, focusing on strict adherence to NetworkManager documentation, proper D-Bus introspection, and atomic handoff.

## Key Issues Identified

1. **Not following NetworkManager documentation strictly** - Initially used outdated `master/slave` terminology instead of `controller`
2. **Wrong parameter names** - Used `ifname` instead of `conn.interface`
3. **IP configuration placement** - IP should be on the ovs-interface connection, not a separate connection
4. **Auto-generated connection names** - NetworkManager creates names like `ovs-bridge-bridge0`, not just `bridge0`
5. **Unmanaged bridges** - Existing OVS bridges without NetworkManager connections show as "unmanaged"

## NetworkManager Documentation Examples

### Example 20: Creating a Bridge with a single internal Interface
```bash
$ nmcli conn add type ovs-bridge conn.interface bridge0
Connection 'ovs-bridge-bridge0' (d10fc64d-1d48-4394-a1b8-e1aea72f27d5) successfully added.
$ nmcli conn add type ovs-port conn.interface port0 controller bridge0
Connection 'ovs-port-port0' (5ae22bae-bba4-4815-9ade-7e635633e1f0) successfully added.
$ nmcli conn add type ovs-interface port-type ovs-port conn.interface iface0 \
  controller port0 ipv4.method manual ipv4.address 192.0.2.1/24
Connection 'ovs-interface-iface0' (3640d2a1-a2fd-4718-92f1-cffadb5b6cdc) successfully added.
```

### Example 21: Adding a Linux interface to a Bridge
```bash
$ nmcli conn add type ovs-port conn.interface port1 controller bridge0
Connection 'ovs-port-port1' (67d041eb-8e7b-4458-afee-a1d07c9c4552) successfully added.
$ nmcli conn add type ethernet conn.interface eth0 controller port1
Connection 'ovs-slave-eth0' (d459c45c-cf78-4c1c-b4b7-505e71379624) successfully added.
```

## Key Concepts

### 1. Connection Hierarchy
```
ovs-bridge (controller)
    ├── ovs-port (controlled by bridge)
    │   └── ovs-interface (controlled by port) - IP goes here
    └── ovs-port (controlled by bridge)
        └── ethernet (controlled by port) - Physical interface
```

### 2. IP Configuration
- IP addresses are configured on the `ovs-interface` connection
- Physical interfaces (ethernet) have their IP removed when enslaved
- Use introspection to migrate IP from active connections

### 3. Atomic Handoff
- Create ALL connections before activation
- Activate only the bridge
- NetworkManager brings up controlled connections atomically

## Final Implementation

### Main Install Script (install_nm_compliant.sh)

The script now:
1. Uses `controller` instead of deprecated `master/slave`
2. Uses `conn.interface` instead of `ifname`
3. Creates connections with auto-generated names
4. Introspects IP from active connections
5. Places IP on ovs-interface connection
6. Handles atomic activation properly

### Key Functions

#### create_ovs_bridge()
```bash
# Following NetworkManager documentation Example 20 EXACTLY
nmcli conn add type ovs-bridge conn.interface "$bridge_name"
```

#### create_internal_port()
```bash
# Create OVS port (Example 20, line 2)
nmcli conn add type ovs-port conn.interface "port0" controller "$bridge_name"

# Create OVS interface with IP (Example 20, line 3)
nmcli conn add type ovs-interface port-type ovs-port \
    conn.interface "iface0" controller "port0" \
    ipv4.method manual ipv4.address "$IP"
```

#### create_uplink_port()
```bash
# Create OVS port following Example 21
nmcli conn add type ovs-port conn.interface "port1" controller "$bridge_name"

# Add Linux interface to bridge (Example 21)
nmcli conn add type ethernet conn.interface "$uplink_if" controller "port1"
```

### Supporting Scripts

1. **check_network_safety.sh** - Checks which interfaces are safe to modify
2. **setup_ovs_bridge_nm.sh** - Reference implementation with safety checks
3. **fix_unmanaged_ovs.sh** - Creates NM connections for existing OVS bridges
4. **recreate_ovs_clean.sh** - Complete cleanup and recreation
5. **validate_nm_compliance.sh** - Validates NetworkManager compliance
6. **diagnose_ovs_bridge.sh** - Troubleshoots activation issues

## Common Issues and Solutions

### 1. Bridge Activation Timeout
**Cause**: Missing OVS service, incomplete topology, or IP conflicts
**Solution**: Ensure OVS is running, all connections created before activation

### 2. Unmanaged Bridges
**Cause**: Bridges created with `ovs-vsctl` directly
**Solution**: Use `fix_unmanaged_ovs.sh` to create NM connections

### 3. SSH Disconnection
**Cause**: Modifying the interface used for SSH
**Solution**: Use safety checks, create bridge without uplink first

### 4. Wrong Connection Names
**Cause**: Script using custom names instead of auto-generated
**Solution**: Use auto-generated names like `ovs-bridge-bridge0`

## Testing Commands

```bash
# Check device status
nmcli device status

# Check connections
nmcli connection show

# Check OVS state
ovs-vsctl show

# Activate bridge
nmcli connection up ovs-bridge-bridge0

# Diagnose issues
./scripts/diagnose_ovs_bridge.sh --bridge bridge0
```

## Migration from Old Setup

For systems with existing OVS bridges:

```bash
# Option 1: Fix in place
sudo ./scripts/fix_unmanaged_ovs.sh

# Option 2: Clean recreation
sudo ./scripts/recreate_ovs_clean.sh ovsbr0 192.168.1.10/24 192.168.1.1
```

## Important Notes

1. **NetworkManager version matters** - This uses the latest syntax with `controller`
2. **Auto-generated names** - NM creates names like `ovs-bridge-<interface>`
3. **IP on interface** - IP configuration goes on ovs-interface, not separate
4. **Atomic activation** - Only activate the bridge, NM handles the rest
5. **Safety first** - Always check which interface is used for SSH

## Code Repository Structure

```
nm-monitor/
├── src/
│   ├── nmcli_dyn.rs        # Dynamic port management
│   ├── nm_bridge.rs         # Bridge creation (old syntax)
│   └── nm_controller.rs     # Bridge creation (new syntax)
├── scripts/
│   ├── install_nm_compliant.sh      # Main install script
│   ├── check_network_safety.sh      # Safety checks
│   ├── diagnose_ovs_bridge.sh       # Diagnostics
│   ├── fix_unmanaged_ovs.sh         # Fix unmanaged bridges
│   ├── recreate_ovs_clean.sh        # Clean recreation
│   └── validate_nm_compliance.sh    # Validation
└── docs/
    └── NetworkManager_Compliance.md  # Detailed documentation
```

## Conclusion

The key to proper OVS bridge creation with NetworkManager is:
1. Follow the documentation syntax exactly
2. Understand the connection hierarchy
3. Place IP on the correct connection (ovs-interface)
4. Create all connections before activation
5. Let NetworkManager handle the atomic transition

The implementation now strictly follows NetworkManager documentation and handles all edge cases properly.