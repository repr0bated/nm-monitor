# Quick Reference - OVS Port Agent

## Current Setup Status

✅ **ovsbr0**: 172.16.0.10/24, uplink: enp2s0
✅ **ovsbr1**: 10.200.0.1/24, NAT to WiFi (wlp4s0)
✅ **Agent**: Running, monitoring ovsbr0

## Essential Commands

### Check Status
```bash
# OVS bridges
sudo ovs-vsctl show
sudo ovs-vsctl list-ports ovsbr0
sudo ovs-vsctl list-ports ovsbr1

# Interfaces
ip addr show ovsbr0
ip addr show ovsbr1

# Agent
sudo systemctl status ovs-port-agent
sudo journalctl -u ovs-port-agent -f

# Routing
ip route show
sudo iptables -t nat -L -n -v | grep -A5 POSTROUTING
```

### Add Container Port Manually
```bash
# Add veth interface to ovsbr0
sudo ovs-vsctl add-port ovsbr0 vethXXXXXX

# Remove port
sudo ovs-vsctl del-port ovsbr0 vethXXXXXX
```

### Agent Control
```bash
# Restart
sudo systemctl restart ovs-port-agent

# Stop
sudo systemctl stop ovs-port-agent

# View config
cat /etc/ovs-port-agent/config.toml

# View ledger
sudo tail -f /var/lib/ovs-port-agent/ledger.jsonl
```

### Make iptables Persistent
```bash
sudo apt install iptables-persistent
sudo netfilter-persistent save
sudo netfilter-persistent reload
```

### Recreate Setup from Scratch
```bash
# Delete bridges
sudo ovs-vsctl del-br ovsbr0
sudo ovs-vsctl del-br ovsbr1

# Create ovsbr0 (physical)
sudo ovs-vsctl add-br ovsbr0
sudo ovs-vsctl add-port ovsbr0 enp2s0
sudo ip addr add 172.16.0.10/24 dev ovsbr0
sudo ip link set ovsbr0 up
sudo ip route add default via 172.16.0.1

# Create ovsbr1 (NAT)
sudo ovs-vsctl add-br ovsbr1
sudo ip addr add 10.200.0.1/24 dev ovsbr1
sudo ip link set ovsbr1 up

# Enable forwarding
sudo sysctl -w net.ipv4.ip_forward=1

# NAT rules
sudo iptables -t nat -A POSTROUTING -s 10.200.0.0/24 -o wlp4s0 -j MASQUERADE
sudo iptables -A FORWARD -i ovsbr1 -o wlp4s0 -j ACCEPT
sudo iptables -A FORWARD -i wlp4s0 -o ovsbr1 -m state --state RELATED,ESTABLISHED -j ACCEPT

# Restart agent
sudo systemctl restart ovs-port-agent
```

## Network Layout

```
┌─────────────────────────────────────────┐
│         Physical Network                │
│         172.16.0.0/24                  │
│         GW: 172.16.0.1                 │
└────────────┬────────────────────────────┘
             │
        ┌────┴─────┐
        │  enp2s0  │  (uplink)
        └────┬─────┘
             │
        ┌────┴─────┐
        │  ovsbr0  │  172.16.0.10/24
        │ (bridge) │
        └────┬─────┘
             │
       ┌─────┴──────┐
       │ containers │ (future veth ports)
       └────────────┘


┌─────────────────────────────────────────┐
│         WiFi Network                    │
│         192.168.0.0/24                 │
│         GW: 192.168.0.1                │
└────────────┬────────────────────────────┘
             │
        ┌────┴─────┐
        │  wlp4s0  │  (WiFi, no bridge)
        └────┬─────┘
             │
          [NAT/MASQUERADE]
             │
        ┌────┴─────┐
        │  ovsbr1  │  10.200.0.1/24
        │ (bridge) │
        └────┬─────┘
             │
       ┌─────┴──────┐
       │ containers │ (future veth ports)
       └────────────┘
```

## Agent Behavior

The agent monitors `/sys/class/net` for interfaces matching:
- `veth-*`
- `veth*`
- `tap*`

When detected, it would:
1. Create NetworkManager dynamic connections (currently not working)
2. Update `/etc/network/interfaces` managed block
3. Log action to ledger

## Files & Locations

- **Binary**: `/usr/local/bin/ovs-port-agent`
- **Config**: `/etc/ovs-port-agent/config.toml`
- **Service**: `/etc/systemd/system/ovs-port-agent.service`
- **Ledger**: `/var/lib/ovs-port-agent/ledger.jsonl`
- **Source**: `/git/nm-monitor/`

## Troubleshooting

### Bridge not found
```bash
# List all bridges
sudo ovs-vsctl list-br

# If missing, recreate (see above)
```

### No internet from ovsbr1
```bash
# Check forwarding
sysctl net.ipv4.ip_forward

# Check NAT rules
sudo iptables -t nat -L -n -v

# Check routing
ip route show table all
```

### Agent not detecting interfaces
```bash
# Check interface names
ls /sys/class/net/

# Check config prefixes
grep include_prefixes /etc/ovs-port-agent/config.toml

# Watch logs
sudo journalctl -u ovs-port-agent -f
```

### D-Bus service not working
```bash
# Check if registered
gdbus call --system --dest dev.ovs.PortAgent1 \
  --object-path /dev/ovs/PortAgent1 \
  --method dev.ovs.PortAgent1.ping

# If fails, D-Bus registration issue (known bug)
# Use ovs-vsctl manually instead
```

## What's Working / Not Working

✅ Bridges created and active
✅ Routing and NAT functional
✅ Agent binary running
✅ OVS 3.5.0 operational

❌ NetworkManager OVS integration incomplete
❌ D-Bus service not registering
❌ Dynamic NM connection creation broken

**Bottom line**: Manual `ovs-vsctl` works perfectly. Use that for now.
