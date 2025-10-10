# nm-monitor (OVS Port Agent)

Rust agent that keeps container veth/tap interfaces attached as OVS ports on a bridge (default `ovsbr0`), surfaces ports in `/etc/network/interfaces` for Proxmox visibility, exposes a D‑Bus API, and writes an append‑only hash‑chain ledger of actions.

Works on Proxmox VE and generic Debian/Ubuntu with Open vSwitch.

## Features
- Proactively create container interfaces with `vi{VMID}` naming at container creation time
- Automatic NetworkManager connection creation for proper vi{VMID} interface names
- Updates a bounded block in `/etc/network/interfaces` with OVSPort stanzas
- D‑Bus service `dev.ovs.PortAgent1` (create/remove container interfaces, ping)
- Journald logging, CLI helpers
- Append‑only ledger of actions at `/var/lib/ovs-port-agent/ledger.jsonl`
- FUSE filesystem integration for Proxmox GUI visibility

## Quickstart
```bash
git clone https://github.com/repr0bated/nm-monitor.git
cd nm-monitor
cargo build --release
sudo ./scripts/install.sh --bridge ovsbr0 --system
```

Optional: create a second empty bridge `ovsbr1` during install:
```bash
sudo ./scripts/install.sh --with-ovsbr1 --system
```

## Configuration
File: `/etc/ovs-port-agent/config.toml`

```toml
# Name of the Open vSwitch bridge to manage
bridge_name = "ovsbr0"

# Interfaces file to update for Proxmox visibility
interfaces_path = "/etc/network/interfaces"

# Interface name prefixes to include as container ports
include_prefixes = ["veth-", "tap", "veth"]

# Debounce interval for periodic reconcile (ms)
debounce_ms = 500

# Tag for the bounded block in /etc/network/interfaces
managed_block_tag = "ovs-port-agent"

# Naming template (≤15 chars after sanitize); variables {container}, {index}
naming_template = "veth-{container}-eth{index}"

# Enable renaming from raw veth to the template name
enable_rename = false

# Optional helper to resolve container name (advanced)
# container_name_cmd = "/usr/local/bin/container-name-from-netns {ifname}"

# Ledger file (append-only JSONL with hash chain)
ledger_path = "/var/lib/ovs-port-agent/ledger.jsonl"
```

## Systemd
```bash
sudo systemctl enable --now ovs-port-agent
sudo systemctl status ovs-port-agent --no-pager
sudo journalctl -u ovs-port-agent -f
```

## D‑Bus usage
Service name: `dev.ovs.PortAgent1`
Object path: `/dev/ovs/PortAgent1`
Interface: `dev.ovs.PortAgent1`

```bash
# Health check
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.ping

# List ports on the managed bridge
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.list_ports

# Create/remove container interfaces
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.create_container_interface 'veth-123-eth0' 'container-123' 100
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.remove_container_interface 'vi100'
```

## CLI helpers
```bash
# Show vi{VMID} naming example
./target/release/ovs-port-agent name my-container 0

# Create container interface with proper vi{VMID} naming
./target/release/ovs-port-agent create-interface veth-123-eth0 container-123 100

# Remove container interface
./target/release/ovs-port-agent remove-interface vi100

# List container interfaces via CLI
./target/release/ovs-port-agent list

# Print NetworkManager root introspection (debug helper)
sudo ./target/release/ovs-port-agent introspect
```

## Proxmox notes
- `ovsbr0` can replace `vmbr0` as the host bridge. Move host IP to `ovsbr0` and enslave the NIC.
- Proxmox GUI will display ports listed in the bounded block of `/etc/network/interfaces`.
- Container interfaces are automatically named using the `vi{VMID}` format (e.g., `vi100`, `vi101`).

## Roadmap
- Container lifecycle integration (create/remove interfaces via container runtime hooks)
- Enhanced Proxmox integration with VMID resolution
- Optional base OpenFlow programming (NORMAL + punt mesh CIDRs to LOCAL)
- More D‑Bus methods (flow control, config reload, batch operations)

## Development
```bash
cargo fmt && cargo clippy && cargo build
```

See [AGENTS.md](AGENTS.md) for contributor guidelines covering project layout, coding style, and review expectations.

## License
Apache-2.0
LLM RULES: NetworkManager + OVS Bridge (D-Bus Introspection Version)
0) Scope

All LLM actions involving Open vSwitch (OVS) bridges must be done through NetworkManager using D-Bus.
No direct shell commands (ovs-vsctl, ip, nmcli) are allowed for configuration — only for verification.

1) Canonical Source

The only source of truth is /home/claude/docs/networkmanager.dev/.

No assumptions outside the documentation.

D-Bus introspection replaces parsing CLI output.

2) Hard Prohibitions

Never use ovs-vsctl to create, modify, or delete bridges, ports, or interfaces.

Never use ip addr or nmcli dev show for configuration discovery.

Never use master/slave; use controller.

Never assign IPs to physical NICs when enslaved. IP must go on the ovs-interface.

3) Correct Hierarchy

ovs-bridge (controller)
├─ ovs-port (controller = bridge)
│ └─ ovs-interface (controller = port) ← IP/DNS/Gateway here
└─ ovs-port (controller = bridge)
└─ ethernet (controller = port) ← enslaved NIC, no IP

4) Creation Sequence

Create each layer with AddConnection2() using proper settings:

connection.type = ovs-bridge

connection.interface-name = bridge0

ipv4.method = disabled for the bridge

Add ovs-port and ovs-interface connections linked via controller

Add physical ethernet connection linked to an ovs-port

Activate only the bridge using ActivateConnection().

5) IP Placement

Extract current IP info from D-Bus, not shell commands.
Use the following interfaces:

org.freedesktop.NetworkManager.Device.Ip4Config

org.freedesktop.NetworkManager.IP4Config.Addresses

org.freedesktop.NetworkManager.IP4Config.Gateway

org.freedesktop.NetworkManager.IP4Config.Nameservers

Apply the extracted IP, gateway, and DNS to the ovs-interface.

6) D-Bus Introspection Method

To inspect live data:

busctl get-property org.freedesktop.NetworkManager /org/freedesktop/NetworkManager/Devices/N org.freedesktop.NetworkManager.Device Ip4Config
busctl introspect org.freedesktop.NetworkManager /org/freedesktop/NetworkManager/IP4Config/M

The same can be done with godbus in Go code for automated retrieval.

7) Atomic Activation

Create all connections first, then activate only the bridge.
NetworkManager will bring up all child connections automatically.
Do not manually bring up ports or interfaces.

8) Verification

Confirm via D-Bus:

Device state = ACTIVATED

IP4Config has correct address

Active connection matches ovs-interface

Optional read-only verification:

nmcli -t -f NAME,DEVICE,STATE con show

ip addr show dev <bridge>

9) Checkpoint and Rollback

Always create a checkpoint before changes:
nmcli general checkpoint create
If something fails:
nmcli general checkpoint rollback

10) Logging

All scripts must log to /var/log/nm-ovs-setup.log.
Use tee to log stdout and stderr.
Always record pre and post state with:

nmstatectl show

busctl tree org.freedesktop.NetworkManager

11) Unmanaged Devices

If unmanaged OVS devices exist, they must be either:

Recreated under NetworkManager control, or

Deleted and rebuilt correctly.

Final state must have no unmanaged OVS objects.

12) Compliance Checklist

IP only on ovs-interface

Physical NIC enslaved, no IP

Bridge active

Introspection shows correct IP4Config

No use of ovs-vsctl

Activation is atomic and reversible via checkpoint
