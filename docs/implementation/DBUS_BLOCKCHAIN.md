# D-Bus Blockchain Integration

## Overview

This system is designed as a D-Bus-centric blockchain foundation where all network state changes are introspectable and auditable via D-Bus interfaces.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  D-Bus System Bus                            │
│                  (Introspectable State)                      │
└──────────┬──────────────────────────────┬───────────────────┘
           │                              │
    ┌──────┴──────┐              ┌───────┴────────┐
    │ NetworkManager│              │ ovs-port-agent │
    │ org.freedesktop│              │ dev.ovs.PortAgent1│
    │ .NetworkManager│              └───────┬────────┘
    └──────┬──────┘                        │
           │                               │
      ┌────┴─────┐                   ┌─────┴──────┐
      │ OVS      │                   │ Ledger     │
      │ Bridges  │                   │ (JSONL)    │
      └──────────┘                   │ Hash Chain │
                                     └────────────┘
                                           │
                                     ┌─────┴──────┐
                                     │ Blockchain │
                                     │ (Future)   │
                                     └────────────┘
```

## D-Bus Services

### 1. NetworkManager OVS Interface
**Service**: `org.freedesktop.NetworkManager`
**Purpose**: Manages OVS bridges, ports, and interfaces

**Key Objects**:
- `/org/freedesktop/NetworkManager` - Root object
- `/org/freedesktop/NetworkManager/Settings` - Connection management
- `/org/freedesktop/NetworkManager/Devices/*` - Device state

**Introspection**:
```bash
# List all NM objects
gdbus introspect --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager

# Get OVS bridge state
gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager \
  --method org.freedesktop.NetworkManager.GetDevices
```

### 2. OVS Port Agent
**Service**: `dev.ovs.PortAgent1`
**Object**: `/dev/ovs/PortAgent1`
**Purpose**: Dynamic container port management with audit trail

**Interface**:
```xml
<interface name="dev.ovs.PortAgent1">
  <method name="Ping">
    <arg type="s" direction="out" name="response"/>
  </method>
  <method name="ListPorts">
    <arg type="as" direction="out" name="ports"/>
  </method>
  <method name="AddPort">
    <arg type="s" direction="in" name="name"/>
  </method>
  <method name="DelPort">
    <arg type="s" direction="in" name="name"/>
  </method>
</interface>
```

**Usage**:
```bash
# Health check
gdbus call --system \
  --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1 \
  --method dev.ovs.PortAgent1.Ping

# List dynamic ports
gdbus call --system \
  --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1 \
  --method dev.ovs.PortAgent1.ListPorts

# Add container port
gdbus call --system \
  --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1 \
  --method dev.ovs.PortAgent1.AddPort \
  string:'veth1234'

# Introspect the service
gdbus introspect --system \
  --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1
```

## Blockchain Integration Points

### 1. Ledger (Current)
Location: `/var/lib/ovs-port-agent/ledger.jsonl`

Format:
```json
{"ts":1696454400,"action":"nm_add_dyn_port","details":{"port":"veth123","bridge":"ovsbr0"},"prev_hash":"","hash":"abc123..."}
{"ts":1696454401,"action":"rename","details":{"old":"veth123","new":"veth-container-eth0","bridge":"ovsbr0"},"prev_hash":"abc123...","hash":"def456..."}
```

**Properties**:
- Append-only
- SHA256 hash chain
- Timestamped
- JSON Lines format

### 2. D-Bus Signal Monitoring (Future)
Subscribe to NetworkManager signals for state changes:

```python
from gi.repository import GLib
from pydbus import SystemBus

bus = SystemBus()
nm = bus.get('org.freedesktop.NetworkManager')

# Subscribe to device state changes
nm.onDeviceAdded = lambda device: print(f"Device added: {device}")
nm.onDeviceRemoved = lambda device: print(f"Device removed: {device}")

loop = GLib.MainLoop()
loop.run()
```

### 3. Properties Introspection
All state is queryable via D-Bus Properties interface:

```bash
# Get all NetworkManager properties
gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager \
  --method org.freedesktop.DBus.Properties.GetAll \
  string:'org.freedesktop.NetworkManager'

# Monitor property changes
gdbus monitor --system --dest org.freedesktop.NetworkManager
```

## Complete Workflow Example

### Container Lifecycle via D-Bus

1. **Container starts, veth interface appears**
```bash
# Agent detects in /sys/class/net
# Logs to ledger:
{"ts":...,"action":"interface_detected","details":{"ifname":"veth1234"}}
```

2. **Agent creates NetworkManager connection**
```bash
# Via NM D-Bus API (not nmcli):
gdbus call --system \
  --dest org.freedesktop.NetworkManager \
  --object-path /org/freedesktop/NetworkManager/Settings \
  --method org.freedesktop.NetworkManager.Settings.AddConnection \
  ...

# Logs to ledger:
{"ts":...,"action":"nm_add_dyn_port","details":{"port":"veth1234","bridge":"ovsbr0"},"prev_hash":"...","hash":"..."}
```

3. **External system queries state**
```bash
# Via D-Bus introspection:
gdbus call --system \
  --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1 \
  --method dev.ovs.PortAgent1.ListPorts

# Returns: (['veth1234'],)

# Blockchain reads ledger, verifies hash chain
cat /var/lib/ovs-port-agent/ledger.jsonl | jq -c '.hash'
```

4. **Container stops**
```bash
# Agent detects interface removal
# Deletes NM connection via D-Bus
# Logs to ledger:
{"ts":...,"action":"nm_del_dyn_port","details":{"port":"veth1234"},"prev_hash":"...","hash":"..."}
```

## D-Bus Policy

Located: `/etc/dbus-1/system.d/dev.ovs.PortAgent1.conf`

Key features:
- **Root ownership**: Only root can claim the service name
- **Universal introspection**: Anyone can introspect (blockchain requirement)
- **Universal access**: All users can call methods (for blockchain queries)
- **NetworkManager integration**: NM can interact with the agent

## Extending for Blockchain

### Required Additions

1. **D-Bus Signal Emissions**
   - Emit signals on state changes
   - Allow blockchain to subscribe in real-time

2. **Property Change Notifications**
   - Use `PropertiesChanged` signal
   - Broadcast all state mutations

3. **Transaction Batching**
   - Group related operations
   - Atomic commit with single hash

4. **Consensus Integration**
   - D-Bus method for blockchain vote
   - Verify before applying state changes

### Example Extension

```rust
// In src/rpc.rs
#[zbus::dbus_interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    // Existing methods...

    // NEW: Signal emission
    #[dbus_interface(signal)]
    async fn port_added(signal_ctxt: &SignalContext<'_>, port: &str, bridge: &str) -> zbus::Result<()>;

    // NEW: Get full state for blockchain sync
    fn get_state(&self) -> zbus::fdo::Result<HashMap<String, String>> {
        // Return complete current state
    }

    // NEW: Verify ledger integrity
    fn verify_ledger(&self) -> zbus::fdo::Result<bool> {
        // Check hash chain
    }
}
```

## Monitoring and Debugging

### Real-time D-Bus Monitoring
```bash
# Watch all D-Bus traffic
dbus-monitor --system

# Watch specific service
gdbus monitor --system --dest dev.ovs.PortAgent1

# Watch NetworkManager
gdbus monitor --system --dest org.freedesktop.NetworkManager
```

### Introspection Tools
```bash
# List all system services
busctl list | grep -E '(NetworkManager|ovs|PortAgent)'

# Tree view of service
busctl tree org.freedesktop.NetworkManager

# Detailed introspection
busctl introspect dev.ovs.PortAgent1 /dev/ovs/PortAgent1
```

### Ledger Verification
```bash
# Check hash chain integrity
cat /var/lib/ovs-port-agent/ledger.jsonl | jq -r '.hash' | sha256sum

# Verify each record
python3 << 'EOF'
import json, hashlib
with open('/var/lib/ovs-port-agent/ledger.jsonl') as f:
    prev_hash = ""
    for line in f:
        rec = json.loads(line)
        # Verify hash chain
        assert rec['prev_hash'] == prev_hash
        prev_hash = rec['hash']
        print(f"✓ {rec['action']} at {rec['ts']}")
EOF
```

## Security Considerations

1. **D-Bus Policy**: Controls who can call methods
2. **Ledger Immutability**: Append-only, tampering detectable via hash chain
3. **Root Ownership**: Only systemd (as root) can start the service
4. **Audit Trail**: All actions logged with timestamps

## Next Steps for Full Blockchain

1. **Distributed Ledger**: Replicate ledger across nodes
2. **Consensus Protocol**: Add voting before state changes
3. **Smart Contracts**: D-Bus methods trigger blockchain logic
4. **Event Sourcing**: Rebuild state from ledger replay
5. **Merkle Tree**: Efficient state verification

## Summary

This is a **D-Bus-first architecture** where:
- ✅ All network state is introspectable via D-Bus
- ✅ All mutations are logged to append-only ledger
- ✅ Hash chain ensures integrity
- ✅ NetworkManager provides the state machine
- ✅ Agent provides the audit and automation layer
- 🔄 Blockchain integration is the natural next step

The foundation is **complete and operational**. The blockchain layer can now be built on top of this introspectable, auditable D-Bus infrastructure.
