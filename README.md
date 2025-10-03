# nm-monitor (OVS Port Agent)

Rust agent that keeps container veth/tap interfaces attached as OVS ports on a bridge (default `ovsbr0`), surfaces ports in `/etc/network/interfaces` for Proxmox visibility, exposes a D‑Bus API, and writes an append‑only hash‑chain ledger of actions.

Works on Proxmox VE and generic Debian/Ubuntu with Open vSwitch.

## Features
- Attach/detach container ports to `ovsbr0` (configurable) via `ovs-vsctl`
- Optional renaming to a template like `veth-{container}-eth{index}` (≤ 15 chars)
- Updates a bounded block in `/etc/network/interfaces` with OVSPort stanzas
- D‑Bus service `dev.ovs.PortAgent1` (list/add/del ports, ping)
- Journald logging, CLI helpers
- Append‑only ledger of actions at `/var/lib/ovs-port-agent/ledger.jsonl`

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

# Add/delete a port
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.add_port 'container_eth0'
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.del_port 'container_eth0'
```

## CLI helpers
```bash
# Show sanitized name example
./target/release/ovs-port-agent name my-container 0

# List OVS ports via CLI
./target/release/ovs-port-agent list

# Print NetworkManager root introspection (debug helper)
sudo ./target/release/ovs-port-agent introspect
```

## Proxmox notes
- `ovsbr0` can replace `vmbr0` as the host bridge. Move host IP to `ovsbr0` and enslave the NIC.
- Proxmox GUI will display ports listed in the bounded block of `/etc/network/interfaces`.
- If you enable renaming, prefer names like `veth-<container>-ethN`; keep to ≤15 chars.

## Roadmap
- Switch from periodic scan to rtnetlink subscription with debounce
- Container name/idx resolution for template
- Optional base OpenFlow programming (NORMAL + punt mesh CIDRs to LOCAL)
- More D‑Bus methods (flow control, config reload)

## Development
```bash
cargo fmt && cargo clippy && cargo build
```

## License
Apache-2.0
