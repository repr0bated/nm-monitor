# Current System State & Architecture - 2025-10-18

## What We Built Today

### Architecture Evolution
- **Removed**: NetworkManager dependency
- **Removed**: systemd-networkd control
- **Removed**: ovs-port-agent as a service (now CLI-only)
- **Added**: Direct OVSDB D-Bus control
- **Added**: Btrfs subvolume for OVSDB state snapshots

### Current Services Running
```
openvswitch-switch.service    - Core OVS (enabled)
ovsdb-server.service          - OVSDB daemon (enabled)
ovs-vswitchd.service          - OVS forwarding (enabled)
ovsdb-dbus-wrapper.service    - D-Bus interface to OVSDB (enabled)
ovsdb-fuse-mount.service      - FUSE filesystem view (enabled, optional)
ovs-boot-setup.service        - Boot persistence (enabled)
```

### Boot Persistence Architecture

**File**: `/etc/ovs-boot-state.json`
```json
{
  "network": {
    "bridges": [
      {
        "name": "vmbr0",
        "ports": ["ens1"],
        "dhcp": true
      }
    ]
  }
}
```

**Service**: `/etc/systemd/system/ovs-boot-setup.service`
- Runs: `ovs-port-agent apply-state /etc/ovs-boot-state.json`
- After: openvswitch-switch.service, ovsdb-server.service
- Creates bridge + gets IP via DHCP at boot

### Btrfs Snapshot Architecture

**OVSDB Location**: `/etc/openvswitch/` (Btrfs subvolume)
- Contains: `conf.db` (OVSDB database)
- Snapshotable for instant state capture/restore

**To snapshot state**:
```bash
btrfs subvolume snapshot /etc/openvswitch /var/lib/ovsdb-snapshots/TIMESTAMP
```

**To restore state**:
```bash
systemctl stop openvswitch-switch
btrfs subvolume delete /etc/openvswitch
btrfs subvolume snapshot /var/lib/ovsdb-snapshots/SNAPSHOT-NAME /etc/openvswitch
systemctl start openvswitch-switch
```

### D-Bus Architecture

**Hierarchy**:
```
ovs-port-agent (CLI tool)
    ↓
NetStatePlugin (reads JSON config)
    ↓
ovsdb-dbus-wrapper (Rust D-Bus service)
    ↓
OVSDB via Unix socket + D-Bus
    ↓
Btrfs snapshots for safe read access
```

**D-Bus Service**: `org.openvswitch.ovsdb.wrapper`
- Methods: create-bridge, add-port, delete-bridge
- Uses Btrfs snapshots to avoid direct OVSDB writes
- Provides introspection via D-Bus queries

### FUSE Mount (Optional)

**Mount Point**: `/var/lib/ovs-port-agent/ovsdb/`
**Type**: Read-Write (changed from RO today)
**Structure**:
```
/var/lib/ovs-port-agent/ovsdb/
├── by-uuid/bridges/     - Bridges by UUID
├── by-name/bridges/     - Bridges by name (symlinks)
├── aliases/             - User aliases
└── desired.json         - Write state here (not fully implemented)
```

**Note**: FUSE is optional for introspection only. Core functionality uses D-Bus directly.

### Binary Locations

```
/usr/local/sbin/ovs-port-agent        - Main CLI tool (Rust)
/usr/local/bin/ovs-port-agent         - Symlink to above
/usr/local/bin/ovsdb-dbus-wrapper     - D-Bus service (Rust)
/usr/local/bin/ovsdb-fuse-mount       - FUSE mount (Rust)
/usr/bin/ovs-port-agent               - Old version (can remove)
```

### Network Configuration

**Physical Interface**: `ens1`
**IP Assignment**: DHCP on `vmbr0` bridge
**Bridge**: `vmbr0` (OVS bridge, contains ens1)

**At boot**:
1. OVSDB loads existing config from `/etc/openvswitch/conf.db`
2. `ovs-boot-setup.service` runs apply-state
3. Creates bridge if doesn't exist (idempotent)
4. Adds ens1 to bridge
5. Runs DHCP to get IP

### Known Issues Fixed Today

1. ✅ ovs-port-agent service failing → Removed service, made it CLI-only
2. ✅ ovsdb-fuse-mount JSON parse error → Fixed `list_bridges_json()` to parse plain text
3. ✅ FUSE mount was RO → Changed to RW (write methods need implementation)
4. ✅ Bridge not persistent at boot → Created ovs-boot-setup.service
5. ✅ Uplink doesn't get IP at boot → Added DHCP to boot script

### CLI Commands

**Create bridge**:
```bash
ovs-port-agent create-bridge vmbr0
```

**Add port**:
```bash
ovs-port-agent add-port vmbr0 ens1
```

**Apply declarative state**:
```bash
ovs-port-agent apply-state /etc/ovs-boot-state.json
```

**Query current state**:
```bash
ovs-port-agent query-state
ovs-port-agent query-state network
```

**List OVS ports**:
```bash
ovs-port-agent list
```

### Next Steps / TODO

- [ ] Test reboot to verify bridge persistence
- [ ] Verify uplink gets IP via DHCP at boot
- [ ] Consider removing FUSE services if not needed
- [ ] Implement container veth auto-binding hooks
- [ ] Document Btrfs snapshot workflow for state management

### Important Notes

1. **No Netplan**: Using direct OVS commands + apply-state
2. **No NetworkManager**: Removed completely
3. **No systemd-networkd**: Removed from ovs-port-agent control
4. **Pure D-Bus**: All state management via OVSDB D-Bus interface
5. **Btrfs native**: State snapshots use Btrfs subvolumes, no JSON overhead

### Architecture Philosophy

**Declarative → D-Bus → Btrfs**
- JSON config files define desired state
- D-Bus provides introspection and control
- Btrfs provides instant snapshots with zero overhead
- FUSE provides filesystem view (optional)

No heavyweight network managers, just clean D-Bus + Btrfs primitives.

---
Last updated: 2025-10-18 21:41 EDT
