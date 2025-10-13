# Declarative State Management Examples

This directory contains example YAML files for declarative system state management.

## Network State Examples

### Basic OVS Bridges with DHCP
```bash
# Apply configuration
sudo ovs-port-agent apply-state config/examples/network-ovs-bridges.yaml

# Query current state
sudo ovs-port-agent query-state network

# Show diff before applying
sudo ovs-port-agent show-diff config/examples/network-ovs-bridges.yaml
```

### Static IP for VPS
```bash
# Apply static IP configuration
sudo ovs-port-agent apply-state config/examples/network-static-ip.yaml
```

## D-Bus Interface

You can also use D-Bus directly:

```bash
# Apply state via D-Bus
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.ApplyState \
  string:"$(cat config/examples/network-ovs-bridges.yaml)"

# Query current network state
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.QueryState \
  string:"network"

# Show diff
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.ShowDiff \
  string:"$(cat config/examples/network-ovs-bridges.yaml)"
```

## State File Format

State files use YAML with the following structure:

```yaml
version: 1

# Each top-level key maps to a plugin
network:
  interfaces:
    - name: <interface-name>
      type: <ethernet|ovs-bridge|bridge>
      ports: [<optional list of ports>]
      ipv4:
        enabled: <true|false>
        dhcp: <true|false>
        address:
          - ip: <ip-address>
            prefix: <cidr-prefix>
        gateway: <gateway-ip>
        dns: [<dns-servers>]
      controller: <optional-bridge-name>
```

## Features

- **Atomic Operations**: All changes applied atomically with automatic rollback on failure
- **Blockchain Audit**: Every state change recorded in immutable ledger
- **Idempotent**: Apply same state multiple times safely
- **Diff Calculation**: See what will change before applying
- **Verification**: Automatic verification after apply
- **Rollback**: Checkpoint/rollback capability for safe changes

