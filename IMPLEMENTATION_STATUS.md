# Implementation Status - Local Machine

## Summary

Successfully implemented OVS bridges with the ovs-port-agent on the local machine with a hybrid NetworkManager/manual approach.

## What's Working

### 1. OVS Bridges Created
- **ovsbr0**: Bridge with enp2s0 as uplink, IP 172.16.0.10/24
- **ovsbr1**: Internal-only bridge, IP 10.200.0.1/24

```bash
$ sudo ovs-vsctl show
2abf2c70-4326-47b1-b9fd-d850ad40fbeb
    Bridge ovsbr1
        Port ovsbr1
            Interface ovsbr1
                type: internal
    Bridge ovsbr0
        Port ovsbr0
            Interface ovsbr0
                type: internal
        Port enp2s0
            Interface enp2s0
    ovs_version: "3.5.0"
```

### 2. Routing Configuration
- IP forwarding enabled
- NAT configured from ovsbr1 (10.200.0.0/24) through WiFi (wlp4s0)
- iptables MASQUERADE and FORWARD rules active

```bash
$ ip route show
default via 172.16.0.1 dev enx00e04c682bbd proto dhcp src 172.16.0.2 metric 100
default via 192.168.0.1 dev wlp4s0 proto dhcp src 192.168.0.2 metric 600
10.200.0.0/24 dev ovsbr1 proto kernel scope link src 10.200.0.1
172.16.0.0/24 dev ovsbr0 proto kernel scope link src 172.16.0.10
```

### 3. OVS Port Agent
- Binary built and installed at `/usr/local/bin/ovs-port-agent`
- systemd service active and enabled
- Config at `/etc/ovs-port-agent/config.toml`
- Monitoring bridge ovsbr0 for dynamic container ports

```bash
$ sudo systemctl status ovs-port-agent
● ovs-port-agent.service - OVS container port agent (Rust)
     Loaded: loaded (/etc/systemd/system/ovs-port-agent.service; enabled)
     Active: active (running)
```

## Known Issues

### 1. NetworkManager OVS Integration
- NM created connections but didn't create actual OVS bridges
- Had to create bridges manually with `ovs-vsctl add-br`
- ovs-interface connection won't activate (controller/port mismatch)

**Root Cause**: NetworkManager's OVS plugin doesn't automatically create bridges; it expects them to exist or creates them lazily. The install script's approach doesn't fully work on all systems.

**Workaround Applied**: Created bridges and configured IPs manually, NM tracks them as unmanaged.

### 2. D-Bus Service Not Registered
- Agent runs but D-Bus service `dev.ovs.PortAgent1` isn't accessible
- No error messages in logs
- RPC module starts but registration might be failing silently

**TODO**: Debug D-Bus permissions or registration logic.

### 3. WiFi Bridging Not Possible
- As expected, WiFi interface wlp4s0 cannot be bridged due to 802.11 limitations
- Used NAT/routing instead (correct approach)

## Configuration Files

### /etc/ovs-port-agent/config.toml
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

### iptables NAT rules (not persistent yet)
```bash
sudo iptables -t nat -A POSTROUTING -s 10.200.0.0/24 -o wlp4s0 -j MASQUERADE
sudo iptables -A FORWARD -i ovsbr1 -o wlp4s0 -j ACCEPT
sudo iptables -A FORWARD -i wlp4s0 -o ovsbr1 -m state --state RELATED,ESTABLISHED -j ACCEPT
```

## How It Works Now

1. **ovsbr0** - Physical bridge
   - Bridges enp2s0 (ethernet)
   - IP: 172.16.0.10/24
   - Gateway: 172.16.0.1
   - Purpose: Direct L2 connectivity for containers that need real network access

2. **ovsbr1** - NAT bridge
   - Internal-only (no physical ports)
   - IP: 10.200.0.1/24
   - Routes through WiFi (wlp4s0) via NAT
   - Purpose: Isolated containers with internet access via WiFi

3. **Agent** - Port monitor
   - Watches for veth/tap interfaces in `/sys/class/net`
   - Would create dynamic NM connections (currently broken)
   - Updates `/etc/network/interfaces` for Proxmox visibility
   - Logs to append-only ledger

## Next Steps to Fix

### Make iptables persistent
```bash
sudo apt install iptables-persistent
sudo netfilter-persistent save
```

### Debug D-Bus service
Check permissions, verify zbus registration, ensure system bus config allows the service name.

### Fix NetworkManager integration
Either:
1. Pre-create bridges with `ovs-vsctl`, then create NM connections pointing to them
2. Use newer NM syntax (controller instead of master/slave)
3. Switch to pure `ovs-vsctl` management (skip NM entirely for OVS)

### Make bridges persistent
Create systemd service or NetworkManager connection files that recreate bridges on boot.

## Testing Agent Functionality

Once you have container veth interfaces:

```bash
# Agent should detect and attach them
sudo journalctl -u ovs-port-agent -f

# Check OVS ports
sudo ovs-vsctl list-ports ovsbr0

# Check dynamic NM connections (if D-Bus works)
nmcli connection show | grep dyn-
```

## Differences from Server Setup

- No docker/netmaker containers (yet)
- WiFi routing instead of WiFi bridging
- enp2s0 uplink instead of physical server NIC
- Hybrid manual/NM approach due to NM integration issues
