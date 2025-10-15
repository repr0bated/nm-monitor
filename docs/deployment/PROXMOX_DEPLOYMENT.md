# Proxmox OVS Container Interface Integration - Deployment Guide

## Overview

This document describes the fixes implemented to make container network interfaces visible and editable in the Proxmox web GUI when using Open vSwitch (OVS) bridges managed by NetworkManager.

## Problem Statement

When using OVS bridges for LXC container networking with NetworkManager managing the actual network state, the Proxmox GUI had several issues:

1. Container interfaces (`vi*`) were not appearing in `/etc/network/interfaces`
2. When they did appear, they showed as "Unknown" type in Proxmox GUI
3. The Proxmox GUI would not allow editing network configuration
4. The internal bridge interface (`ovsbr0-if`) was not visible

## Root Causes

### Issue 1: Missing Container Interfaces
**Problem**: Container interfaces weren't showing in `/etc/network/interfaces` when containers were stopped.

**Cause**: The code scans `/sys/class/net/` for interfaces matching prefixes (`veth`, `tap`). When containers are stopped, their veth interfaces don't exist, so nothing was detected.

**Solution**: This is expected behavior - the code correctly only shows running container interfaces. Stopped containers don't have network interfaces in the kernel.

### Issue 2: "Unknown" Interface Type in Proxmox
**Problem**: Container ports showed as "Unknown" instead of "OVS IntPort" in Proxmox GUI.

**Cause**:
- Used `allow-ovs` instead of `allow-{bridge}` for container ports
- Used `OVSPort` instead of `OVSIntPort` for virtual interfaces
- Proxmox checks if interface name matches physical NIC regex pattern (`(?:nic|if)\d+|eth\d+|en[^:.]+`) before recognizing `OVSPort` type
- Virtual interfaces like `vi9000` don't match this pattern, so must use `OVSIntPort` instead

**Solution**:
- Changed container ports to use `OVSIntPort` type
- Removed `allow-{bridge}` lines from ports (only needed on bridge itself)
- Added `auto` flag for interfaces that should start automatically

### Issue 3: Missing Internal Interface
**Problem**: The OVS bridge internal interface (`ovsbr0-if`) wasn't listed in Proxmox.

**Cause**: The code didn't write the internal interface to `/etc/network/interfaces`.

**Solution**: Added automatic generation of internal interface configuration.

## Code Changes

### File: `src/interfaces.rs`

#### Change 1: Add Internal Bridge Interface
```rust
// Internal interface for the bridge
let internal_if = format!("{}-if", bridge);
block.push_str(&format!(
    "auto {i}\niface {i} inet manual\n    ovs_type OVSIntPort\n    ovs_bridge {b}\n\n",
    i = internal_if,
    b = bridge
));
```

#### Change 2: Fix Uplink Port Format
```rust
// Uplink physical port (if specified)
if let Some(uplink_iface) = uplink {
    block.push_str(&format!(
        "iface {u} inet manual\n    ovs_bridge {b}\n    ovs_type OVSPort\n\n",
        b = bridge,
        u = uplink_iface
    ));
}
```
**Changes**: Removed `allow-{bridge}` line from uplink port.

#### Change 3: Fix Container Port Format
```rust
// Container ports
if port_names.is_empty() {
    block.push_str("# No container OVS ports detected.\n");
} else {
    for name in port_names {
        block.push_str(&format!(
            "auto {n}\niface {n} inet manual\n    ovs_type OVSIntPort\n    ovs_bridge {b}\n\n",
            n = name,
            b = bridge
        ));
    }
}
```
**Changes**:
- Changed `OVSPort` → `OVSIntPort`
- Removed `allow-ovs` line
- Added `auto` flag

## Deployment Steps

### 1. Build the Updated Code
```bash
cd /git/nm-monitor
cargo build --release
```

### 2. Stop the Service
```bash
sudo systemctl stop ovs-port-agent
```

### 3. Install Updated Binary
```bash
sudo cp target/release/ovs-port-agent /usr/local/bin/
```

### 4. Start the Service
```bash
sudo systemctl start ovs-port-agent
```

### 5. Verify Configuration
```bash
# Check that interfaces file was updated
cat /etc/network/interfaces | grep -A30 "BEGIN ovs-port-agent"

# Verify service is running
sudo systemctl status ovs-port-agent
```

## Expected Configuration Output

After deployment, `/etc/network/interfaces` should contain:

```
# BEGIN ovs-port-agent
# Managed by ovs-port-agent. Do not edit.
# This is for Proxmox GUI visibility only.
# NetworkManager manages the actual state via D-Bus.

auto ovsbr0
allow-ovs ovsbr0
iface ovsbr0 inet manual
    ovs_type OVSBridge
    ovs_ports enp2s0 vi9000 vi9002 vi9003

auto ovsbr0-if
iface ovsbr0-if inet manual
    ovs_type OVSIntPort
    ovs_bridge ovsbr0

iface enp2s0 inet manual
    ovs_bridge ovsbr0
    ovs_type OVSPort

auto vi9000
iface vi9000 inet manual
    ovs_type OVSIntPort
    ovs_bridge ovsbr0

auto vi9002
iface vi9002 inet manual
    ovs_type OVSIntPort
    ovs_bridge ovsbr0

auto vi9003
iface vi9003 inet manual
    ovs_type OVSIntPort
    ovs_bridge ovsbr0

# END ovs-port-agent
```

## Verification in Proxmox GUI

1. Navigate to: **Node → System → Network**
2. Verify the following interfaces appear:

| Interface | Type | Active | Comments |
|-----------|------|--------|----------|
| `ovsbr0` | OVS Bridge | Yes | Main bridge |
| `ovsbr0-if` | OVS IntPort | Yes | Internal interface (harmless, can be left) |
| `enp2s0` | OVS Port | Yes | Physical uplink |
| `vi9000` | OVS IntPort | Yes | Container 9000 port |
| `vi9002` | OVS IntPort | Yes | Container 9002 port |
| `vi9003` | OVS IntPort | Yes | Container 9003 port |

3. Verify you can click "Edit" on any interface without errors
4. Verify no interfaces show as "Unknown" type

## Configuration Reference

### Config File: `/etc/ovs-port-agent/config.toml`

Example configuration:
```toml
bridge_name = "ovsbr0"
interfaces_path = "/etc/network/interfaces"
include_prefixes = ["veth", "tap"]
debounce_ms = 500
managed_block_tag = "ovs-port-agent"
naming_template = "vi{container}"
enable_rename = true
ledger_path = "/var/lib/ovs-port-agent/ledger.jsonl"
uplink = "enp2s0"
```

### Key Configuration Options

- **`include_prefixes`**: Interface name prefixes to monitor (`["veth", "tap"]`)
- **`naming_template`**: How to rename interfaces (`"vi{container}"` → `vi9000`)
- **`enable_rename`**: Whether to rename veth interfaces (recommended: `true`)
- **`uplink`**: Physical interface attached to OVS bridge (e.g., `"enp2s0"`)

## Proxmox Interface Type Reference

| `ovs_type` Value | Proxmox Display | Use Case |
|------------------|-----------------|----------|
| `OVSBridge` | OVS Bridge | The main OVS bridge |
| `OVSPort` | OVS Port | Physical interfaces attached to bridge |
| `OVSIntPort` | OVS IntPort | Virtual/internal ports (containers, bridge IPs) |
| `OVSBond` | OVS Bond | Bonded interfaces |

**Important**: Virtual interfaces (veth, tap, renamed ports) must use `OVSIntPort`, not `OVSPort`, or Proxmox will show them as "Unknown".

## Starting/Stopping Containers

### When a Container Starts
1. Proxmox/LXC creates a veth pair (e.g., `vethABC123`)
2. `ovs-port-agent` detects the new interface
3. Agent renames it (e.g., `vethABC123` → `vi9000`)
4. Agent creates NetworkManager OVS port connection
5. Agent updates `/etc/network/interfaces` with new port
6. Interface appears in Proxmox GUI as "OVS IntPort"

### When a Container Stops
1. Proxmox/LXC removes the veth pair
2. `ovs-port-agent` detects interface removal
3. Agent removes NetworkManager connection
4. Agent updates `/etc/network/interfaces` (removes port entry)
5. Interface disappears from Proxmox GUI

## Troubleshooting

### Container Interfaces Not Appearing

**Check if containers are running:**
```bash
sudo pct list
sudo pct status 9000
```

**Check for veth interfaces:**
```bash
ls /sys/class/net/ | grep -E 'veth|vi'
```

**Check OVS bridge status:**
```bash
sudo ovs-vsctl show
```

**Check service logs:**
```bash
sudo journalctl -u ovs-port-agent -f
```

### Interfaces Show as "Unknown" in Proxmox

**Verify interface configuration:**
```bash
cat /etc/network/interfaces | grep -A5 "vi9000"
```

Should show:
```
auto vi9000
iface vi9000 inet manual
    ovs_type OVSIntPort
    ovs_bridge ovsbr0
```

**If it shows `OVSPort` instead of `OVSIntPort`**, the code needs updating.

### Bridge Device Not Found When Starting Container

**Error:** `bridge 'ovsbr0' does not exist`

**Solution:** Create the bridge device:
```bash
sudo ip link add ovsbr0 type bridge
sudo ip link set ovsbr0 up
```

Or verify NetworkManager has created the bridge:
```bash
nmcli connection up ovsbr0
```

## Additional Notes

### Why Not Remove `ovsbr0-if`?

The internal interface (`ovsbr0-if`) is used if you want to assign an IP address to the bridge itself (e.g., for management access). While containers don't attach to it directly, it's harmless to leave in the Proxmox GUI and may be useful for future configuration.

### Why Use NetworkManager Instead of `/etc/network/interfaces`?

NetworkManager provides:
- Dynamic management via D-Bus
- Automatic state reconciliation
- Better integration with modern Linux systems
- API for programmatic control

The `/etc/network/interfaces` file is maintained **only for Proxmox GUI visibility**. The actual network state is managed by NetworkManager.

### Interface Naming Convention

The default naming template `vi{container}` creates predictable names:
- Container 9000 → `vi9000`
- Container 9002 → `vi9002`
- Container 203 → `vi203`

This makes it easy to identify which interface belongs to which container.

## Commit Reference

**Commit**: `Fix Proxmox GUI interface display`

**Changes**:
- `src/interfaces.rs`: Updated interface generation for Proxmox compatibility
- Changed container ports from `OVSPort` to `OVSIntPort`
- Added internal bridge interface (`ovsbr0-if`)
- Removed unnecessary `allow-{bridge}` lines
- Added `auto` flag to auto-start interfaces

## Related Files

- `/git/nm-monitor/src/interfaces.rs` - Interface file generation
- `/git/nm-monitor/src/netlink.rs` - Interface detection and reconciliation
- `/etc/systemd/system/ovs-port-agent.service` - Systemd service file
- `/etc/ovs-port-agent/config.toml` - Configuration file
- `/etc/network/interfaces` - Proxmox-readable network config
- `/var/lib/ovs-port-agent/ledger.jsonl` - Audit log

## Support

For issues or questions, check:
- Project repository: `/git/nm-monitor/`
- Service logs: `sudo journalctl -u ovs-port-agent`
- Status documentation: `/git/nm-monitor/STATUS.md`
- Implementation status: `/git/nm-monitor/IMPLEMENTATION_STATUS.md`
