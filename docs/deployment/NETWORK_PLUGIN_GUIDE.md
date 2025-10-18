# Network Plugin User Guide

## Overview

The **Network State Plugin** provides declarative management of network interfaces, OVS bridges, and IP configuration through `systemd-networkd` and Open vSwitch.

## Features

### ✅ Implemented

- **OVS Bridge Management**
  - Create/delete OVS bridges
  - Attach/detach ports to bridges
  - Configure bridge IPs (static or DHCP)

- **Interface Configuration**
  - Static IPv4 addresses with gateway and DNS
  - DHCP configuration
  - Bridge controllers (enslaved interfaces)

- **Validation**
  - Interface name validation (max 15 chars)
  - IP address format validation
  - Configuration consistency checks
  - OVS availability verification

- **State Management**
  - Query current network state
  - Calculate diffs between current and desired state
  - Apply changes atomically
  - Rollback support via checkpoints

## Usage

### 1. Query Current State

```bash
# Query all network state
ovs-port-agent query-state --plugin network

# Via D-Bus
busctl call dev.ovs.PortAgent1 /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1 query_state "s" "network"
```

### 2. Show Diff (Dry Run)

```bash
# Preview changes without applying
ovs-port-agent show-diff config/examples/network-ovs-bridges.yaml

# Via D-Bus
busctl call dev.ovs.PortAgent1 /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1 show_diff "s" "$(cat config.yaml)"
```

### 3. Apply State

```bash
# Apply declarative configuration
ovs-port-agent apply-state config/examples/network-ovs-bridges.yaml

# Via D-Bus
busctl call dev.ovs.PortAgent1 /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1 apply_state "s" "$(cat config.yaml)"
```

## Configuration Examples

### Example 1: Simple Isolated Bridge

```yaml
version: 1

network:
  interfaces:
    - name: ovsbr-test
      type: ovs-bridge
      ipv4:
        enabled: true
        dhcp: false
        address:
          - ip: 10.99.99.1
            prefix: 24
```

**Use case**: Testing, isolated container network

### Example 2: Bridge with Uplink (VPS)

```yaml
version: 1

network:
  interfaces:
    # OVS bridge with uplink
    - name: ovsbr0
      type: ovs-bridge
      ports:
        - eth0  # Physical uplink
      ipv4:
        enabled: true
        dhcp: false
        address:
          - ip: 192.168.1.10
            prefix: 24
        gateway: 192.168.1.1
        dns:
          - 1.1.1.1
          - 8.8.8.8
    
    # Physical interface (enslaved)
    - name: eth0
      type: ethernet
      controller: ovsbr0
      ipv4:
        enabled: false
```

**Use case**: Production VPS with OVS bridge on uplink

### Example 3: Multiple Bridges

```yaml
version: 1

network:
  interfaces:
    # Primary bridge with uplink
    - name: ovsbr0
      type: ovs-bridge
      ports:
        - enp2s0
      ipv4:
        enabled: true
        dhcp: true
    
    - name: enp2s0
      type: ethernet
      controller: ovsbr0
      ipv4:
        enabled: false
    
    # Isolated Docker bridge
    - name: ovsbr1
      type: ovs-bridge
      ipv4:
        enabled: true
        address:
          - ip: 172.18.0.1
            prefix: 16
```

**Use case**: Production server with separate uplink and container networks

## Configuration Schema

### Interface Types

- `ethernet` - Physical or virtual ethernet interface
- `ovs-bridge` - Open vSwitch bridge
- `ovs-port` - OVS port (future)
- `bridge` - Linux bridge (future)

### Interface Configuration

```yaml
interfaces:
  - name: string              # Interface name (max 15 chars)
    type: string              # Interface type
    ports: [string]           # (Optional) Ports to attach (OVS bridges only)
    controller: string        # (Optional) Bridge to enslave to
    ipv4:
      enabled: bool           # Enable IPv4
      dhcp: bool              # (Optional) Use DHCP
      address:                # (Optional) Static addresses
        - ip: string          # IPv4 address
          prefix: number      # Prefix length (0-32)
      gateway: string         # (Optional) Default gateway
      dns: [string]           # (Optional) DNS servers
    ipv6:                     # (Optional) IPv6 config
      enabled: bool
      dhcp: bool
```

## Validation Rules

### Interface Names

- **Max Length**: 15 characters (Linux kernel limit)
- **Valid Characters**: Alphanumeric, dash (`-`), underscore (`_`)
- **Examples**:
  - ✅ `ovsbr0`, `eth0`, `veth-test`, `br_docker`
  - ❌ `ovsbr-really-long-name` (too long), `eth@0` (invalid char)

### IP Addresses

- **IPv4 Format**: `x.x.x.x` where x is 0-255
- **Prefix Range**: 0-32
- **Static IP**: Must specify at least one address if `dhcp: false`
- **DHCP**: Cannot specify addresses with `dhcp: true`

### Configuration Constraints

- **OVS bridges** cannot be enslaved to other bridges
- **Enslaved interfaces** should not have IP configuration (will warn)
- **Ports** can only be specified on OVS bridges

## Testing

### Automated Test Suite

Run the comprehensive test suite:

```bash
sudo ./scripts/test-network-plugin.sh
```

This tests:
1. Current state querying
2. Diff calculation
3. Bridge creation
4. IP configuration
5. Bridge deletion

### Manual Testing

```bash
# 1. Build release binary
cargo build --release

# 2. Test with simple isolated bridge (safe)
sudo ./target/release/ovs-port-agent \
  apply-state config/examples/test-ovs-simple.yaml

# 3. Verify bridge exists
sudo ovs-vsctl br-exists ovsbr-test && echo "✓ Bridge created"

# 4. Check IP configuration
ip addr show ovsbr-test

# 5. Query state
sudo ./target/release/ovs-port-agent query-state --plugin network

# 6. Cleanup (delete bridge)
cat > /tmp/cleanup.yaml <<EOF
version: 1
network:
  interfaces: []
EOF

sudo ./target/release/ovs-port-agent apply-state /tmp/cleanup.yaml
rm /tmp/cleanup.yaml
```

## Troubleshooting

### Error: "OVS is not available or not running"

**Solution**:
```bash
# Install OVS
sudo apt-get install openvswitch-switch

# Start OVS
sudo systemctl start openvswitch-switch
sudo systemctl enable openvswitch-switch
```

### Error: "Interface name exceeds 15 character limit"

**Solution**: Shorten interface names to 15 characters or less.

### Error: "Failed to reload systemd-networkd"

**Solution**:
```bash
# Check networkd status
sudo systemctl status systemd-networkd

# Start if not running
sudo systemctl start systemd-networkd
sudo systemctl enable systemd-networkd
```

### Warning: "Interface has IP configuration but is enslaved"

**Explanation**: Enslaved interfaces should not have IPs (bridge gets the IP).

**Solution**: Remove `ipv4.enabled` or set to `false` for enslaved interfaces.

## Architecture

### Component Flow

```
User Config (YAML)
  ↓
StateManager.apply_state()
  ↓
NetworkStatePlugin
  ├─→ validate_interface_config()
  ├─→ check_ovs_available()
  ├─→ create_ovs_bridge() ──→ ovs-vsctl add-br
  ├─→ attach_ovs_port()   ──→ ovs-vsctl add-port
  ├─→ write_config_files() ──→ /etc/systemd/network/*.network
  └─→ reload_networkd()    ──→ networkctl reload
```

### File Locations

- **Config Files**: `/etc/systemd/network/10-<interface>.network`
- **NetDev Files**: `/etc/systemd/network/10-<interface>.netdev` (OVS bridges)
- **Ledger**: `/var/log/ovs-port-agent/ledger.json` (audit trail)

### D-Bus Integration

- **Service**: `dev.ovs.PortAgent1`
- **Object Path**: `/dev/ovs/PortAgent1`
- **Methods**:
  - `apply_state(state_yaml: string) → result: string`
  - `query_state(plugin: string) → state: string`
  - `show_diff(desired_yaml: string) → diff: string`

## Production Deployment

### 1. Install and Enable

```bash
# Install binary
sudo cp target/release/ovs-port-agent /usr/local/bin/

# Install systemd service
sudo cp systemd/ovs-port-agent.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable ovs-port-agent
sudo systemctl start ovs-port-agent
```

### 2. Prepare Configuration

```bash
# Create your production config
sudo mkdir -p /etc/ovs-port-agent
sudo vim /etc/ovs-port-agent/network.yaml
```

### 3. Apply Configuration

```bash
# Test with diff first
sudo ovs-port-agent show-diff /etc/ovs-port-agent/network.yaml

# Apply when ready
sudo ovs-port-agent apply-state /etc/ovs-port-agent/network.yaml
```

### 4. Verify

```bash
# Check bridges
sudo ovs-vsctl show

# Check networkd status
sudo networkctl status

# Query state
sudo ovs-port-agent query-state --plugin network
```

## Safety Features

1. **Idempotent Operations**: Safe to re-apply same config
2. **Validation Before Apply**: Catches errors early
3. **Checkpoint/Rollback**: Can restore previous state
4. **Blockchain Audit**: All changes logged immutably
5. **Dry Run Support**: Preview changes with `show-diff`

## Limitations

### Current Limitations

- **IPv6**: Parsing implemented, but limited configuration support
- **Complex Routing**: Only basic gateway support
- **VLANs**: Not yet implemented
- **Bonding**: Not yet implemented
- **systemd-networkd Native OVS**: Uses ovs-vsctl + systemd-networkd hybrid

### Future Enhancements

- Advanced routing (policy routing, multiple tables)
- VLAN tagging on OVS ports
- Link aggregation / bonding
- QoS configuration
- OVS flow rules
- IPv6 full support
- Network namespaces

## See Also

- [STATE_MANAGER_ARCHITECTURE.md](STATE_MANAGER_ARCHITECTURE.md) - Plugin architecture
- [DBUS_BLOCKCHAIN.md](DBUS_BLOCKCHAIN.md) - D-Bus API and audit ledger
- [NetworkManager_Compliance.md](NetworkManager_Compliance.md) - Why systemd-networkd

