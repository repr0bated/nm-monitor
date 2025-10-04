# OVS Port Agent - Recovery & Next Steps

## Current Situation

You got lost in a debugging rabbit hole trying to fix NetworkManager syntax. The **original `scripts/install.sh` is actually correct and working** - you just need to get back to basics.

## What You Have (Working Components)

### 1. Working Rust Agent
- **Location**: `/git/nm-monitor/src/`
- **Binary**: `ovs-port-agent`
- **Purpose**: Monitors container veth/tap interfaces and attaches them to OVS bridges via NetworkManager
- **Features**:
  - Watches `/sys/class/net` for new interfaces matching prefixes (veth, tap)
  - Creates NM connections dynamically: `ovs-port` + `ethernet` slave
  - Updates `/etc/network/interfaces` for Proxmox visibility
  - D-Bus service `dev.ovs.PortAgent1`
  - Append-only ledger at `/var/lib/ovs-port-agent/ledger.jsonl`

### 2. Working Install Script
- **Location**: `/git/nm-monitor/scripts/install.sh`
- **What it does**:
  1. Builds the Rust binary
  2. Installs to `/usr/local/bin/ovs-port-agent`
  3. Creates config at `/etc/ovs-port-agent/config.toml`
  4. Installs systemd service
  5. Creates OVS bridges via NetworkManager (proper hierarchy)
  6. Handles uplink migration safely

### 3. The Rabbit Hole (Ignore These)
- `install_nm_compliant.sh` - Debugging script, incomplete
- `fix_unmanaged_ovs.sh` - Diagnostic tool
- `recreate_ovs_clean.sh` - Cleanup script
- All the `compare_nm_syntax.sh`, `test_nm_syntax.sh` etc. - debugging artifacts
- `agent-try.md` - Debugging notes (has useful info but led to confusion)

## Current Server State

### Network Devices
```
enx00e04c682bbd - ethernet, connected (your active connection)
wlp4s0 - wifi, connected
enp2s0 - ethernet, disconnected
ovsbr0/ovsbr1 - OVS bridges, UNMANAGED (created outside NM)
```

### Problem
- OVS bridges exist but are "unmanaged" (created with `ovs-vsctl` directly, not via NetworkManager)
- OVS service may not be running
- No NetworkManager connections for the bridges

## What You Need to Do on Server

### Step 1: Check Prerequisites
```bash
cd /git/nm-monitor

# Is OVS installed and running?
systemctl status openvswitch-switch.service
# or
systemctl status ovs-vswitchd.service

# If not running:
sudo systemctl start openvswitch-switch
sudo systemctl enable openvswitch-switch
```

### Step 2: Clean Up Unmanaged Bridges (Optional)
If you want to start fresh:
```bash
# Delete the unmanaged bridges
sudo ovs-vsctl del-br ovsbr0
sudo ovs-vsctl del-br ovsbr1

# Remove any leftover connections
sudo nmcli connection delete ovsbr0 2>/dev/null || true
sudo nmcli connection delete ovsbr1 2>/dev/null || true
sudo nmcli connection delete ovsbr0-if 2>/dev/null || true
sudo nmcli connection delete ovsbr1-if 2>/dev/null || true
```

### Step 3: Run the Original Install Script

**Basic install (creates ovsbr0 with no IP):**
```bash
sudo ./scripts/install.sh --bridge ovsbr0 --system
```

**With IP address:**
```bash
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --nm-ip 192.168.1.10/24 \
  --nm-gw 192.168.1.1 \
  --system
```

**With uplink (physical interface):**
```bash
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink enp2s0 \
  --nm-ip 80.209.240.244/25 \
  --nm-gw 80.209.240.129 \
  --system
```

**With both bridges:**
```bash
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink enp2s0 \
  --nm-ip 80.209.240.244/25 \
  --nm-gw 80.209.240.129 \
  --with-ovsbr1 \
  --ovsbr1-ip 10.200.0.1/24 \
  --system
```

### Step 4: Verify It's Working
```bash
# Check service
sudo systemctl status ovs-port-agent

# Check bridges
nmcli device status | grep ovs
nmcli connection show | grep ovs

# Check OVS
sudo ovs-vsctl show

# Test D-Bus API
gdbus call --system \
  --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1 \
  --method dev.ovs.PortAgent1.ping
```

## What the Install Script Creates

NetworkManager connection hierarchy:
```
ovsbr0 (ovs-bridge)
  ├── ovsbr0-port-int (ovs-port)
  │   └── ovsbr0-if (ovs-interface) ← IP address goes here
  └── ovsbr0-port-enp2s0 (ovs-port, if --uplink specified)
      └── ovsbr0-uplink-enp2s0 (ethernet)
```

## After Installation

The agent will automatically:
1. Watch for new veth/tap interfaces
2. Create dynamic NM connections: `dyn-port-vethXXX` and `dyn-eth-vethXXX`
3. Attach them to the bridge
4. Update `/etc/network/interfaces`
5. Log all actions to the ledger

## Key Configuration

`/etc/ovs-port-agent/config.toml`:
```toml
bridge_name = "ovsbr0"
interfaces_path = "/etc/network/interfaces"
include_prefixes = ["veth-", "tap", "veth"]
debounce_ms = 500
managed_block_tag = "ovs-port-agent"
naming_template = "veth-{container}-eth{index}"
enable_rename = true
ledger_path = "/var/lib/ovs-port-agent/ledger.jsonl"
```

## Important Notes

1. **Don't use `master/slave`** - The original script uses it but still works. The debugging scripts tried to fix this but got complicated.
2. **The agent uses NetworkManager** - It creates proper NM connections, not raw `ovs-vsctl` commands
3. **Proxmox visibility** - The `/etc/network/interfaces` updates let Proxmox GUI see the ports
4. **SSH safety** - If you're connected via one of the interfaces, test on a different interface first

## If You Need to Start Over

```bash
# Stop the agent
sudo systemctl stop ovs-port-agent

# Clean up
sudo ovs-vsctl del-br ovsbr0 2>/dev/null || true
sudo ovs-vsctl del-br ovsbr1 2>/dev/null || true
for conn in $(nmcli -t -f NAME c show | grep ovs); do
  sudo nmcli connection delete "$conn"
done

# Reinstall
sudo ./scripts/install.sh --bridge ovsbr0 --nm-ip YOUR_IP/MASK --nm-gw YOUR_GW --system
```

## Summary

**Just use `scripts/install.sh`** - it works. The debugging added complexity but didn't improve the core functionality. Get the bridges working first, then worry about syntax updates later if needed.
