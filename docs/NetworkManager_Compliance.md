# NetworkManager OVS Bridge Compliance Guide

This document describes the NetworkManager-compliant implementation for OVS bridge creation and management, strictly following the NetworkManager.dev documentation.

## Overview

The implementation ensures proper OVS bridge creation through NetworkManager with:
- Correct D-Bus interfaces
- Proper introspection support
- Atomic connection handoff
- Compliant nmcli commands

## Key Components

### 1. Bridge Creation Module (`src/nm_bridge.rs`)

Provides NetworkManager-compliant functions for:
- Creating OVS bridges with proper settings
- Managing internal ports for IP assignment
- Handling uplink ports with atomic handoff
- Validating bridge topology

### 2. Dynamic Port Management (`src/nmcli_dyn.rs`)

Updated to follow NetworkManager conventions:
- Proper connection hierarchy (bridge → port → interface/ethernet)
- Correct autoconnect priorities
- Error handling and logging
- Connection existence checks

### 3. D-Bus Service (`src/rpc.rs`)

Enhanced with:
- Comprehensive introspection support
- NetworkManager D-Bus path exploration
- OVS-specific connection introspection
- Policy file for access control (`dbus/dev.ovs.PortAgent1.conf`)

## NetworkManager OVS Topology

According to NetworkManager documentation, OVS connections must follow this hierarchy:

```
ovs-bridge (master)
    ├── ovs-port (slave, type: ovs-bridge)
    │   └── ovs-interface (slave, type: ovs-port) [for internal ports]
    └── ovs-port (slave, type: ovs-bridge)
        └── ethernet (slave, type: ovs-port) [for physical interfaces]
```

### Connection Properties

#### OVS Bridge
- `type`: `ovs-bridge`
- `ovs-bridge.stp`: `no` (recommended for OVS)
- `ovs-bridge.rstp`: `no` (recommended for OVS)
- `ovs-bridge.mcast-snooping-enable`: `yes`
- `connection.autoconnect`: `yes`
- `connection.autoconnect-priority`: `100`

#### OVS Port
- `type`: `ovs-port`
- `connection.master`: `<bridge-name>`
- `connection.slave-type`: `ovs-bridge`
- `connection.autoconnect`: `yes`
- `connection.autoconnect-priority`: `90-95`

#### OVS Interface (Internal)
- `type`: `ovs-interface`
- `connection.master`: `<port-name>`
- `connection.slave-type`: `ovs-port`
- `ovs-interface.type`: `internal`
- `connection.autoconnect`: `yes`
- `connection.autoconnect-priority`: `95`

#### Ethernet (Physical)
- `type`: `ethernet`
- `connection.master`: `<port-name>`
- `connection.slave-type`: `ovs-port`
- `connection.autoconnect`: `yes`
- `connection.autoconnect-priority`: `85`

## Atomic Handoff

NetworkManager handles atomic activation of OVS connections:

1. When activating a bridge, NetworkManager automatically brings up all slave connections
2. Deactivating a master deactivates all slaves
3. This ensures network connectivity is maintained during transitions

## Scripts

### Installation Script (`scripts/install_nm_compliant.sh`)

Compliant installation process:
```bash
./scripts/install_nm_compliant.sh \
    --bridge ovsbr0 \
    --nm-ip 192.168.1.10/24 \
    --nm-gw 192.168.1.1 \
    --uplink eth0
```

### Validation Script (`scripts/validate_nm_compliance.sh`)

Validates NetworkManager compliance:
```bash
./scripts/validate_nm_compliance.sh --bridge ovsbr0
```

Checks:
- Connection types and hierarchy
- Required properties
- Autoconnect settings and priorities
- OVS state consistency
- D-Bus service availability

### Migration Script (`scripts/migrate_to_compliant.sh`)

Migrates existing bridges to compliant configuration:
```bash
./scripts/migrate_to_compliant.sh --bridge ovsbr0 --dry-run
./scripts/migrate_to_compliant.sh --bridge ovsbr0
```

## nmcli Command Examples

### Create OVS Bridge
```bash
nmcli connection add \
    type ovs-bridge \
    con-name ovsbr0 \
    ifname ovsbr0 \
    ovs-bridge.stp no \
    ovs-bridge.rstp no \
    connection.autoconnect yes \
    connection.autoconnect-priority 100
```

### Create Internal Port and Interface
```bash
# Port
nmcli connection add \
    type ovs-port \
    con-name ovsbr0-port-int \
    ifname ovsbr0 \
    connection.master ovsbr0 \
    connection.slave-type ovs-bridge

# Interface
nmcli connection add \
    type ovs-interface \
    con-name ovsbr0-if \
    ifname ovsbr0 \
    connection.master ovsbr0-port-int \
    connection.slave-type ovs-port \
    ovs-interface.type internal
```

### Activate Bridge (Atomic)
```bash
nmcli connection up ovsbr0
```

## D-Bus Introspection

The service supports standard D-Bus introspection:

```bash
# Introspect our service
gdbus introspect --system \
    --dest dev.ovs.PortAgent1 \
    --object-path /dev/ovs/PortAgent1

# Introspect NetworkManager
gdbus introspect --system \
    --dest org.freedesktop.NetworkManager \
    --object-path /org/freedesktop/NetworkManager
```

## Troubleshooting

### Common Issues

1. **Bridge not activating**: Check autoconnect priorities
2. **Slaves not coming up**: Verify master/slave relationships
3. **IP not assigned**: Ensure interface connection is active
4. **Uplink migration fails**: Check for conflicting connections

### Debug Commands

```bash
# Show all OVS connections
nmcli connection show | grep ovs-

# Check connection details
nmcli connection show <connection-name>

# Monitor NetworkManager logs
journalctl -u NetworkManager -f

# Check OVS state
ovs-vsctl show
```

## References

- [NetworkManager OVS Documentation](https://networkmanager.dev/docs/api/latest/nm-settings-ovs.html)
- [D-Bus Specification](https://dbus.freedesktop.org/doc/dbus-specification.html)
- [Open vSwitch Documentation](https://docs.openvswitch.org/)