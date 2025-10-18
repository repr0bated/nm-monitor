# Quick Start: Network Plugin Installation

## 🚀 The Modern Way (Declarative)

### 1. Choose Your Network Config

```bash
# Option A: Safe test (isolated bridge, no uplink)
NETWORK_CONFIG=config/examples/test-ovs-simple.json

# Option B: Single bridge (ovsbr0 only)
NETWORK_CONFIG=config/examples/network-ovsbr0-only.json

# Option C: Production (ovsbr0 + ovsbr1 with uplink)
NETWORK_CONFIG=config/examples/network-ovs-bridges.json

# Option D: VPS with static IP
NETWORK_CONFIG=config/examples/network-static-ip.json
```

### 2. Install

```bash
# Install with your chosen config
sudo ./scripts/install-with-network-plugin.sh \
  --network-config ${NETWORK_CONFIG} \
  --system

# Or add ovsbr1 dynamically (for Docker/containers)
sudo ./scripts/install-with-network-plugin.sh \
  --network-config ${NETWORK_CONFIG} \
  --with-ovsbr1 \
  --system
```

That's it! ✅

---

## What It Does

1. ✅ **Builds** release binary
2. ✅ **Installs** binary, config, systemd service
3. ✅ **Shows diff** (what will change)
4. ✅ **Asks confirmation**
5. ✅ **Applies** network configuration
6. ✅ **Verifies** everything works
7. ✅ **Enables** systemd service (if --system)

---

## Example: Test Installation (Safe)

```bash
# Install with test config (won't affect connectivity)
sudo ./scripts/install-with-network-plugin.sh \
  --network-config config/examples/test-ovs-simple.json \
  --system

# Verify
sudo ovs-vsctl show
sudo ovs-port-agent query-state --plugin network
```

---

## Example: Single Bridge (ovsbr0 only)

```bash
# Install just ovsbr0 (for basic setup)
sudo ./scripts/install-with-network-plugin.sh \
  --network-config config/examples/network-ovsbr0-only.json \
  --system

# Later, add ovsbr1 if needed
sudo ./scripts/install-with-network-plugin.sh \
  --network-config config/examples/network-ovsbr0-only.json \
  --with-ovsbr1 \
  --system
```

---

## Example: Production VPS

### 1. Create Your Config

```bash
sudo mkdir -p /etc/ovs-port-agent
sudo vim /etc/ovs-port-agent/network.json
```

```json
{
  "version": 1,
  "network": {
    "interfaces": [
      {
        "name": "ovsbr0",
        "type": "ovs-bridge",
        "ports": ["eth0"],
        "ipv4": {
          "enabled": true,
          "dhcp": false,
          "address": [
            {
              "ip": "198.51.100.10",
              "prefix": 24
            }
          ],
          "gateway": "198.51.100.1",
          "dns": ["1.1.1.1", "8.8.8.8"]
        }
      },
      {
        "name": "eth0",
        "type": "ethernet",
        "controller": "ovsbr0",
        "ipv4": {
          "enabled": false
        }
      },
      {
        "name": "ovsbr1",
        "type": "ovs-bridge",
        "ipv4": {
          "enabled": true,
          "address": [
            {
              "ip": "172.18.0.1",
              "prefix": 16
            }
          ]
        }
      }
    ]
  }
}
```

### 2. Install

```bash
sudo ./scripts/install-with-network-plugin.sh \
  --network-config /etc/ovs-port-agent/network.json \
  --system
```

---

## Useful Commands

```bash
# Query current network state
sudo ovs-port-agent query-state --plugin network

# Apply new configuration
sudo ovs-port-agent apply-state /path/to/config.json

# Show what would change (dry run)
sudo ovs-port-agent show-diff /path/to/config.json

# Check OVS bridges
sudo ovs-vsctl show

# Service status
sudo systemctl status ovs-port-agent

# View logs
sudo journalctl -u ovs-port-agent -f
```

---

## Comparison: New vs Legacy

### New Way (install-with-network-plugin.sh)
✅ **Simple** - ~300 lines, easy to understand
✅ **Declarative** - JSON config defines desired state
✅ **Safe** - Shows diff, asks confirmation
✅ **Modern** - Uses network plugin architecture
✅ **Clean** - No legacy complexity  

### Legacy Way (install.sh)
⚠️ **Complex** - 1000+ lines  
⚠️ **Imperative** - Manual bridge creation  
⚠️ **Risky** - Direct system modifications  
⚠️ **Old** - NetworkManager-focused  

---

## Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Open vSwitch
sudo apt-get install openvswitch-switch

# Start OVS
sudo systemctl start openvswitch-switch
sudo systemctl enable openvswitch-switch
```

---

## Troubleshooting

### Error: "OVS is not available"
```bash
sudo apt-get install openvswitch-switch
sudo systemctl start openvswitch-switch
```

### Error: "Failed to apply network config"
```bash
# Check OVS status
sudo systemctl status openvswitch-switch

# Check your config syntax
sudo ovs-port-agent show-diff /path/to/config.json

# View detailed errors
sudo journalctl -xe
```

### Want to test without affecting your network?
```bash
# Use the test config (isolated bridge, no uplink)
sudo ./scripts/install-with-network-plugin.sh \
  --network-config config/examples/test-ovs-simple.json \
  --system
```

---

## Full Documentation

- **User Guide**: [docs/NETWORK_PLUGIN_GUIDE.md](docs/NETWORK_PLUGIN_GUIDE.md)
- **Architecture**: [docs/STATE_MANAGER_ARCHITECTURE.md](docs/STATE_MANAGER_ARCHITECTURE.md)
- **D-Bus API**: [DBUS_BLOCKCHAIN.md](DBUS_BLOCKCHAIN.md)

---

## What's Next?

After installation, you have:
- ✅ **Working OVS bridges** configured declaratively
- ✅ **D-Bus RPC service** for programmatic control
- ✅ **CLI tools** for state management
- ✅ **Blockchain ledger** auditing all changes

Ready to use with Netmaker, Docker, or any containerized workload!

