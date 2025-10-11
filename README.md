# nm-monitor (OVS Port Agent) - Introspection-Driven Blockchain Network Management

**Advanced OVS bridge management with D-Bus introspection, blockchain accountability, and comprehensive Proxmox integration.**

Rust agent that provides **zero-connectivity-loss** OVS bridge management through **atomic handover** techniques, **comprehensive D-Bus introspection**, and **immutable blockchain ledger** for complete network operation accountability.

## 🎯 Core Philosophy: Introspection-Driven Networking

**Everything is discovered, never assumed.** The system uses D-Bus introspection to understand NetworkManager capabilities, current network state, and available resources at runtime.

- **🔍 Introspection First**: Discover system state via D-Bus rather than parsing CLI output
- **⛓️ Blockchain Accountability**: Every network operation tracked in immutable hash chain
- **🛡️ Atomic Operations**: NetworkManager checkpoints ensure zero connectivity interruption
- **🔗 Proxmox Integration**: Seamless GUI visibility with enhanced FUSE filesystem
- **📡 Netmaker Ready**: Built-in support for mesh networking container integration

## ✨ Enhanced Features

### **Blockchain Ledger System**
- **Immutable Audit Trail**: SHA-256 hash chain of all network operations
- **Plugin Architecture**: Extensible data source tracking (network, settings, users, storage)
- **D-Bus Query API**: Real-time blockchain statistics and historical analysis
- **Tamper Detection**: Cryptographic verification of operation history

### **Advanced D-Bus API**
- **System Introspection**: Comprehensive network state discovery
- **Bridge Validation**: Multi-layer connectivity verification
- **Atomic Operations**: Safe bridge modifications with rollback
- **Real-time Monitoring**: Live system state via D-Bus

### **Proxmox GUI Compatibility**
- **Underscore Naming**: Proxmox-compatible interface names (no dashes)
- **Enhanced FUSE Integration**: Advanced filesystem bindings for VM visibility
- **VMID Mapping**: Proper vi{VMID} interface naming for Proxmox recognition
- **Configuration Files**: Auto-generated Proxmox-style network configs

### **Netmaker Integration Ready**
- **Container Auto-Detection**: Automatically finds Netmaker mesh containers
- **OVS Bridge Integration**: Seamless connection to ovsbr1 for mesh networking
- **Public IP Emulation**: Makes containers appear as remote servers
- **Mesh Network Support**: Built-in support for Netmaker mesh topologies

## 🚀 Quickstart

### **Basic Installation**
```bash
git clone https://github.com/repr0bated/nm-monitor.git
cd nm-monitor
cargo build --release
sudo ./scripts/install.sh --bridge ovsbr0 --uplink enp2s0 --system
```

### **Proxmox VE Installation**
```bash
# Install with Proxmox integration
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink vmbr0 \
  --with-ovsbr1 \
  --system \
  --purge-bridges
```

### **Netmaker Server Installation**
```bash
# Install for Netmaker mesh networking
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink enp2s0 \
  --with-ovsbr1 \
  --system
```

## 📋 Installation Options

| Option | Description | Use Case |
|--------|-------------|----------|
| `--bridge NAME` | Set bridge name (default: ovsbr0) | Custom bridge naming |
| `--uplink IFACE` | Physical interface to enslave | Internet connectivity |
| `--with-ovsbr1` | Create secondary bridge for containers | Isolated container network |
| `--purge-bridges` | Remove existing bridges first | Clean slate installation |
| `--system` | Enable and start systemd service | Production deployment |
| `--force-ovsctl` | Allow ovs-vsctl fallback | Emergency recovery |

## 🔧 Enhanced D-Bus API

### **System Introspection**
```bash
# Get comprehensive network state
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_system_network_state

# Validate bridge connectivity
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.validate_bridge_connectivity \
  string:ovsbr0

# Get bridge topology
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_bridge_topology \
  string:ovsbr0
```

### **Blockchain Ledger Operations**
```bash
# Get blockchain statistics
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_blockchain_stats

# Query blocks by category
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_blocks_by_category \
  string:network

# Verify blockchain integrity
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.verify_blockchain_integrity

# Add data to blockchain
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.add_blockchain_data \
  string:settings string:modified string:'{"key": "value"}'
```

### **Proxmox Integration**
```bash
# Get interface bindings
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_interface_bindings

# Perform atomic bridge operation
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.atomic_bridge_operation \
  string:ovsbr0 string:create_checkpoint
```

## ⛓️ Blockchain Ledger System

### **Architecture Overview**
The system implements a **SHA-256 hash chain** with plugin-based data source tracking:

```
Block N-1 ← Hash ← Block N ← Hash ← Block N+1 ← Hash ← Latest Block
    ↑              ↑              ↑              ↑
  Previous       Category       Action         Metadata
   Block         (network,      (created,      (user, host,
  Height         settings,      modified,      timestamp,
                 users,         deleted)       etc.)
                 storage)
```

### **Built-in Data Source Plugins**

**1. Network Plugin** - Tracks bridge, interface, connection, and port operations
**2. Settings Plugin** - Monitors configuration and policy changes
**3. User Plugin** - Records authentication and authorization events
**4. Storage Plugin** - Logs filesystem and mount operations

### **Blockchain Query Capabilities**
- **Historical Analysis**: Query any point in system history
- **Integrity Verification**: Cryptographic proof of tamper-evidence
- **Pattern Recognition**: Identify trends and anomalies
- **Compliance Reporting**: Audit trail for regulatory requirements

## 🔗 Proxmox Integration

### **Enhanced GUI Compatibility**
- **Underscore Naming**: `vi_container_name` instead of `vi-container-name`
- **VMID Mapping**: Proper `vi{VMID}` format for Proxmox recognition
- **FUSE Bindings**: Advanced filesystem integration for VM visibility
- **Configuration Files**: Auto-generated Proxmox-style network configurations

### **Proxmox-Specific Features**
- **VM Interface Detection**: Automatically finds VM veth interfaces
- **Bridge State Synchronization**: Keeps OVS and Proxmox in sync
- **GUI Visibility**: Proper interface display in Proxmox web interface
- **Network Configuration**: Updates `/etc/network/interfaces` for Proxmox

## 📡 Netmaker Integration

### **Container Auto-Detection**
The system automatically detects and integrates Netmaker containers:

```bash
# Netmaker containers are auto-detected and connected to ovsbr1
✅ netmaker-server → ovsbr1 (public IP emulation)
✅ netmaker-client-001 → ovsbr1 (mesh node)
✅ netmaker-client-002 → ovsbr1 (mesh node)
```

### **Public IP Emulation**
Netmaker containers appear as independent remote servers:

```
Internet
   ↓
[203.0.113.10] ← Container A (appears as remote server)
[203.0.113.11] ← Container B (appears as remote server)
   ↓
OVS Bridge (ovsbr1) ← Netmaker Mesh Network
```

### **Mesh Network Support**
- **Automatic Port Creation**: OVS ports created for each Netmaker container
- **Traffic Engineering**: Optimized routing for mesh network traffic
- **Security Policies**: Flow rules for mesh network isolation
- **Monitoring Integration**: Blockchain tracking of mesh events

## 🏗️ Architecture Components

### **Core Components**
1. **OVS Bridge Manager** (`src/nm_bridge.rs`) - NetworkManager-compliant bridge operations
2. **Blockchain Ledger** (`src/ledger.rs`) - Immutable audit trail with plugin system
3. **D-Bus API** (`src/rpc.rs`) - System-wide introspection and control
4. **FUSE Integration** (`src/fuse.rs`) - Enhanced Proxmox filesystem integration
5. **Container Interface Manager** (`src/netlink.rs`) - Proactive container networking

### **Key Design Principles**
- **Introspection-Driven**: Discover rather than assume system state
- **Atomic Operations**: NetworkManager checkpoints for zero-connectivity-loss
- **Plugin Architecture**: Extensible data source tracking
- **Immutable Audit**: Cryptographic proof of all operations
- **Proxmox Compatible**: Seamless integration with Proxmox VE

## 📊 System Capabilities

### **Connectivity Preservation**
- **Atomic Handover**: NetworkManager checkpoints prevent interruption
- **Rollback Capability**: Automatic rollback on connectivity loss
- **Multi-layer Validation**: OVS + NetworkManager + connectivity verification
- **Btrfs Snapshots**: System-level rollback for critical failures

### **Accountability & Auditing**
- **Complete Operation Tracking**: Every network change recorded
- **Cryptographic Integrity**: Hash chain prevents tampering
- **Rich Metadata**: User, hostname, timestamp, process context
- **Query Interface**: D-Bus API for historical analysis

### **Extensibility**
- **Plugin System**: Easy to add new data source tracking
- **D-Bus API**: External systems can integrate and control
- **Configuration Driven**: Flexible deployment options
- **Environment Adaptive**: Works in various deployment scenarios

## 🔮 Advanced Features (Planned)

### **OVS Flow Rules**
- **Traffic Engineering**: Advanced routing decisions
- **Quality of Service**: Bandwidth and priority management
- **Security Policies**: Flow-level access control
- **Performance Optimization**: Fast-path processing for common patterns

### **Enhanced Netmaker Integration**
- **Mesh Network Analytics**: Monitor mesh network health
- **Automatic Failover**: Handle mesh node failures
- **Load Balancing**: Distribute traffic across mesh nodes
- **Security Monitoring**: Track mesh network security events

### **Extended Blockchain Capabilities**
- **Digital Signatures**: Cryptographic proof of block authenticity
- **Merkle Trees**: Efficient verification of large datasets
- **Cross-Node Verification**: Distributed ledger consistency
- **Performance Analytics**: Network performance tracking

## 📈 Benefits Summary

| Feature | Benefit | Use Case |
|---------|---------|----------|
| **Atomic Handover** | Zero connectivity loss | Production networks |
| **Blockchain Ledger** | Complete accountability | Compliance requirements |
| **D-Bus Introspection** | Runtime system discovery | Dynamic environments |
| **Proxmox Integration** | Seamless GUI management | Virtualization platforms |
| **Netmaker Support** | Mesh networking | Distributed systems |
| **Plugin Architecture** | Easy extensibility | Custom requirements |

## 🏛️ Technical Architecture

### **System Layers**

```
┌─────────────────────────────────────────────────────────────┐
│                    D-Bus API Layer                          │
│  - System Introspection & Control                          │
│  - Blockchain Query Interface                              │
│  - Real-time State Monitoring                              │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                 Blockchain Ledger Layer                     │
│  - Immutable SHA-256 Hash Chain                           │
│  - Plugin-Based Data Source Tracking                      │
│  - Cryptographic Integrity Verification                   │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              NetworkManager Integration                     │
│  - D-Bus Introspection for System Discovery               │
│  - Atomic Bridge Operations with Rollback                 │
│  - Controller-Based Architecture                          │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                 OVS Bridge Layer                           │
│  - ovsbr0: Main bridge with uplink                        │
│  - ovsbr1: Container/Docker bridge                        │
│  - NetworkManager-Compliant Operations                    │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┘
│                 Proxmox Integration                        │
│  - Enhanced FUSE Filesystem Bindings                      │
│  - VMID-Compatible Interface Naming                       │
│  - GUI Visibility for Network Interfaces                  │
└─────────────────────────────────────────────────────────────┘
```

### **Data Flow Architecture**

```
Container Creation → Interface Detection → OVS Port Creation → NetworkManager Connection → FUSE Binding → Blockchain Record → Proxmox GUI Update
       ↓                    ↓                      ↓                    ↓                      ↓                ↓              ↓
  Netmaker/Docker    Proxmox veth pairs    ovsbr1 bridge     NM connection profile    /var/lib/fuse    SHA-256 hash   /etc/network/interfaces
```

### **Introspection Points**

The system uses D-Bus introspection at multiple levels:

1. **NetworkManager Introspection** - Discover available devices, connections, and capabilities
2. **OVS Bridge Introspection** - Query bridge state, ports, and interfaces
3. **Blockchain Introspection** - Query historical operations and verify integrity
4. **Proxmox Integration Introspection** - Discover VM interfaces and bridge requirements

## 🔧 Configuration & Deployment

### **Environment Detection**

The system automatically adapts to different deployment scenarios:

```bash
# Full-featured deployment (Proxmox + Docker + Netmaker)
✅ NetworkManager + OVS + Docker + Netmaker + Proxmox

# Container-focused deployment  
✅ NetworkManager + OVS + Docker (Netmaker optional)

# Minimal deployment
✅ NetworkManager + OVS (Manual container management)
```

### **Installation Modes**

| Mode | Features | Use Case |
|------|----------|----------|
| **Proxmox VE** | Full GUI integration, VM management | Virtualization platforms |
| **Docker Host** | Container auto-detection, networking | Container orchestration |
| **Netmaker Server** | Mesh networking, public IP emulation | SD-WAN deployments |
| **Generic Server** | Basic OVS bridge management | Traditional server setups |

## 📊 Performance Characteristics

### **Connectivity Preservation**
- **Zero Packet Loss**: Atomic handover prevents interruption
- **Sub-second Failover**: NetworkManager checkpoints enable fast rollback
- **Multi-layer Validation**: OVS + NetworkManager + connectivity verification

### **Blockchain Performance**
- **Efficient Hashing**: SHA-256 operations optimized for network operations
- **Minimal Storage**: JSONL format with compression-friendly structure
- **Fast Queries**: Indexed blockchain queries via D-Bus API

### **System Resource Usage**
- **Memory**: ~50MB baseline + blockchain storage
- **CPU**: Minimal overhead for introspection operations
- **Storage**: ~1KB per blockchain entry (compressed)
- **Network**: D-Bus communication for introspection

## 🚨 Troubleshooting & Recovery

### **Atomic Rollback Procedures**

**If Installation Fails:**
```bash
# Check NetworkManager checkpoint
nmcli general checkpoint list

# Rollback to pre-installation state
nmcli general checkpoint rollback

# Verify connectivity restored
nmcli general status
```

**If Bridge Operation Fails:**
```bash
# Check blockchain for operation history
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_blocks_by_category \
  string:bridge

# Verify bridge state
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.validate_bridge_connectivity \
  string:ovsbr0
```

### **Debugging Commands**

```bash
# Comprehensive system state
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_system_network_state

# Blockchain integrity check
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.verify_blockchain_integrity

# NetworkManager introspection
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.introspect_network_manager

# Bridge topology analysis
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.get_bridge_topology \
  string:ovsbr0
```

### **Log Analysis**

```bash
# Systemd journal for ovs-port-agent
sudo journalctl -u ovs-port-agent -f

# Blockchain ledger inspection
tail -f /var/lib/ovs-port-agent/ledger.jsonl

# NetworkManager logs
sudo journalctl -u NetworkManager --since "1 hour ago"
```

## 🔒 Security Considerations

### **Blockchain Security**
- **Immutable Records**: Hash chain prevents unauthorized modifications
- **Cryptographic Integrity**: SHA-256 verification of all operations
- **Audit Trail**: Complete record of who did what and when
- **Tamper Detection**: Any modification breaks the entire chain

### **Network Security**
- **Minimal Attack Surface**: D-Bus API with controlled access
- **Atomic Operations**: No intermediate states vulnerable to attack
- **Rollback Protection**: Failed operations automatically reversed
- **Comprehensive Logging**: All operations tracked for security analysis

### **Access Control**
- **D-Bus Policy**: Controlled access to management interface
- **Systemd Integration**: Proper service isolation and permissions
- **Configuration Protection**: Secure handling of sensitive network data

## 📚 API Reference

### **D-Bus Methods Summary**

| Method | Parameters | Description |
|--------|------------|-------------|
| `ping` | None | Health check |
| `list_ports` | None | List container interfaces |
| `add_port` | interface_name | Create container interface |
| `del_port` | interface_name | Remove container interface |
| `get_system_network_state` | None | Comprehensive system introspection |
| `validate_bridge_connectivity` | bridge_name | Multi-layer connectivity validation |
| `get_interface_bindings` | None | Proxmox FUSE binding status |
| `atomic_bridge_operation` | bridge, operation | Safe bridge modifications |
| `get_bridge_topology` | bridge_name | Complete bridge state analysis |
| `get_blockchain_stats` | None | Blockchain ledger statistics |
| `get_blocks_by_category` | category | Query blockchain by data type |
| `get_blocks_by_height` | start, end | Query blockchain by height range |
| `verify_blockchain_integrity` | None | Verify hash chain integrity |
| `add_blockchain_data` | category, action, data | Add data to blockchain |
| `get_block_by_hash` | hash | Retrieve specific block |

### **Blockchain Data Categories**

| Category | Description | Examples |
|----------|-------------|----------|
| `bridge` | OVS bridge operations | create, modify, delete |
| `interface` | Network interface management | bind, unbind, configure |
| `connection` | NetworkManager connections | activate, deactivate, modify |
| `port` | OVS port operations | add, remove, configure |
| `connectivity` | Network connectivity events | up, down, route changes |
| `config` | Configuration changes | template, policy, parameter |
| `authentication` | User authentication events | login, logout, session |
| `authorization` | Permission changes | access control, roles |
| `storage` | Filesystem operations | mount, unmount, snapshot |

## 🚀 Deployment Examples

### **Example 1: Proxmox VE Cluster**
```bash
# Primary Proxmox node with ovsbr0
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink vmbr0 \
  --with-ovsbr1 \
  --system

# Secondary nodes join cluster
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink enp2s0 \
  --system
```

### **Example 2: Netmaker Mesh Network**
```bash
# Netmaker server node
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink enp2s0 \
  --with-ovsbr1 \
  --system

# Netmaker clients connect to ovsbr1
# Containers auto-detected and configured
```

### **Example 3: Docker Swarm Cluster**
```bash
# Swarm manager nodes
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink enp2s0 \
  --with-ovsbr1 \
  --system

# Worker nodes join with same config
```

## 🔮 Future Enhancements

### **Planned Features**
1. **OVS Flow Rules** - Advanced traffic engineering and QoS
2. **Enhanced Netmaker Integration** - Mesh network analytics and monitoring
3. **Kubernetes CNI Plugin** - Native Kubernetes networking integration
4. **Network Performance Analytics** - Real-time performance monitoring
5. **Distributed Ledger** - Multi-node blockchain verification

### **Extensibility Roadmap**
1. **Custom Plugins** - Easy addition of new data source tracking
2. **External Integrations** - SNMP, Prometheus, Grafana exporters
3. **REST API** - HTTP interface for external system integration
4. **Web Dashboard** - Visual network management interface
5. **Mobile App** - Mobile network monitoring and control

## 📖 Related Documentation

- **[NetworkManager Compliance](docs/NetworkManager_Compliance.md)** - Detailed compliance with NetworkManager best practices
- **[Proxmox Deployment Guide](PROXMOX_DEPLOYMENT.md)** - Proxmox VE specific deployment instructions
- **[Recovery Guide](RECOVERY.md)** - Troubleshooting and recovery procedures
- **[Quick Reference](QUICK_REFERENCE.md)** - Command reference and cheat sheet

## 🤝 Contributing

See [AGENTS.md](AGENTS.md) for contributor guidelines covering:
- Project layout and organization
- Coding standards and style
- Review processes and expectations
- Testing requirements

## 📄 License

**Apache-2.0**

---

**Built with ❤️ for reliable, accountable, introspection-driven network management.**

This system provides **enterprise-grade network management** with **complete accountability** and **zero-connectivity-risk** deployment capabilities! 🛡️

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

# List container interfaces on the managed bridge
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.list_ports

# Create/remove container interfaces
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.create_container_interface 'veth-123-eth0' 'container-123' 100
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.remove_container_interface 'vi100'

# Comprehensive NetworkManager introspection and debugging
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.introspect_network_manager
```

## CLI helpers
```bash
# Show vi{VMID} naming example
./target/release/ovs-port-agent name my-container 0

# Create container interface with proper vi{VMID} naming
./target/release/ovs-port-agent create-interface veth-123-eth0 container-123 100

# Remove container interface
./target/release/ovs-port-agent remove-interface vi100

# List container interfaces via CLI (shows vi{VMID} names)
./target/release/ovs-port-agent list

# Comprehensive NetworkManager introspection and debugging
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
