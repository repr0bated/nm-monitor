# nm-monitor Refactor Rules

## Core Principle: ELIMINATE NetworkManager

The entire purpose of this refactor is to **REMOVE** the problematic NetworkManager dependency and create a pure OVS + systemd-networkd solution.

## Rule 1: NO NetworkManager
- **NEVER** use NetworkManager D-Bus calls
- **NEVER** use `nmcli` commands
- **NEVER** create NetworkManager connection profiles
- **NEVER** rely on NetworkManager for bridge management
- Remove all `nm_*` modules from codebase

## Rule 2: OVS via OVSDB D-Bus Interface
- **NEVER** use `ovs-vsctl` commands
- Use OVSDB D-Bus interface for bridge operations
- Connect to `org.openvswitch.ovsdb` D-Bus service
- Create bridges via OVSDB JSON-RPC over D-Bus
- systemd-networkd does NOT have native OVS support

## Rule 3: systemd-networkd for State Management
- Use systemd-networkd D-Bus for network state discovery
- Create `.network` files for IP configuration persistence
- Use D-Bus calls to `org.freedesktop.network1` for introspection
- Use D-Bus calls to `org.freedesktop.systemd1` for service management

## Rule 4: Atomic Operations via Backup/Restore
- Create configuration backups before changes
- Test connectivity after changes
- Rollback on failure within timeout
- Use systemd-networkd reload via D-Bus

## Rule 5: OVS Bridge Operations
- Use `ovs-vsctl` for bridge and port operations
- Create OVS bridges with `ovs-vsctl add-br`
- Add ports with `ovs-vsctl add-port`
- Configure via systemd-networkd for persistence

## Rule 6: D-Bus Introspection Priority
- Use systemd-networkd D-Bus for network state discovery
- Use systemd D-Bus for service management
- Direct file system operations only for configuration
- No CLI parsing for state discovery

## Rule 7: Error Recovery
- Always backup existing configuration
- Implement connectivity tests
- Automatic rollback on failure
- Log all operations to blockchain ledger

## Rule 8: Hybrid Architecture
- OVS handles bridge/port operations
- systemd-networkd handles IP configuration
- D-Bus provides state introspection
- Configuration files provide persistence
