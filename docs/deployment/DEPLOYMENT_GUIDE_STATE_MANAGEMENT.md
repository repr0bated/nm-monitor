# nm-monitor (OVS Port Agent) Deployment Guide
## State Management, Component Relationships, and Operational Excellence

**Target Audience:** System Administrators, DevOps Engineers, Deployment Teams, End Users

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Why State Management is Critical](#why-state-management-is-critical)
4. [Component Relationships](#component-relationships)
5. [State Lifecycle](#state-lifecycle)
6. [Deployment Procedures](#deployment-procedures)
7. [Operational Commands](#operational-commands)
8. [Troubleshooting](#troubleshooting)
9. [Advanced Topics](#advanced-topics)

---

## Executive Summary

### What is nm-monitor (OVS Port Agent)?

nm-monitor is a **declarative network state management system** that provides:

- **Zero-downtime network changes** through atomic operations
- **Immutable audit trail** via blockchain ledger
- **Automatic rollback** on failure
- **D-Bus RPC interface** for remote control
- **systemd-networkd integration** for OVS bridge management

### Key Innovation: State-Driven Architecture

Unlike traditional imperative network management tools (run commands, hope for the best), nm-monitor uses a **declarative state model**:

```
Traditional (Imperative):          State-Driven (Declarative):
┌─────────────────────┐           ┌─────────────────────┐
│ Run: ovs-vsctl ...  │           │ Desired State       │
│ Run: ip addr add... │           │ (YAML file)         │
│ Run: systemctl ...  │           │                     │
│ Hope it works! ❌   │           │ System figures      │
└─────────────────────┘           │ out how to get      │
                                   │ there ✅            │
                                   └─────────────────────┘
```

**The fundamental difference:** You declare **what you want**, not **how to do it**. The system handles the complexity, atomicity, and rollback automatically.

---

## Architecture Overview

### System Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                     USER / OPERATOR                             │
│              (CLI, D-Bus, API calls, YAML files)                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    D-BUS RPC INTERFACE                          │
│   - Remote procedure calls                                      │
│   - State query/apply operations                                │
│   - System introspection                                        │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                   STATE MANAGER (Core)                          │
│   - Orchestrates all plugins                                    │
│   - Atomic operation coordination                               │
│   - Checkpoint/rollback management                              │
│   - Diff calculation                                            │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌──────────────────────┬──────────────────────┬──────────────────┐
│   NET PLUGIN         │   NETCFG PLUGIN      │  BLOCKCHAIN      │
│ (Infrastructure)     │  (Configuration)     │   LEDGER         │
│                      │                      │                  │
│ - OVS bridges        │ - Routing tables     │ - Immutable      │
│ - Interfaces         │ - DNS config         │   audit log      │
│ - IP addresses       │ - Flow rules         │ - SHA-256 chain  │
│ - Port memberships   │ - Firewall rules     │ - Tamper proof   │
└──────────────────────┴──────────────────────┴──────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│            SYSTEMD-NETWORKD (Network Backend)                   │
│   - Creates .netdev files (bridge definitions)                 │
│   - Creates .network files (IP config)                         │
│   - Activates network interfaces                               │
│   - Manages systemd units                                      │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                   OPEN VSWITCH (OVS)                           │
│   - Virtual bridge (ovsbr0, ovsbr1)                           │
│   - Port management                                            │
│   - Flow rules                                                 │
│   - VLAN tagging                                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                   LINUX KERNEL                                  │
│   - Network stack                                              │
│   - Netlink interface                                          │
│   - Network namespaces                                         │
└─────────────────────────────────────────────────────────────────┘
```

### How Components Interact

#### 1. **D-Bus ↔ State Manager**

```rust
// D-Bus receives RPC call
dev.ovs.PortAgent1.ApplyState(state_yaml)
        ↓
// Routes to State Manager
state_manager.apply_state(desired_state).await
        ↓
// Returns result to D-Bus caller
ApplyReport { success: true, ... }
```

**Why D-Bus?**
- **System-wide IPC**: Any process can communicate
- **Authentication**: PolicyKit integration for access control
- **Introspection**: Self-documenting API
- **Event-driven**: Signals for state changes

#### 2. **State Manager ↔ Plugins**

```rust
// State Manager orchestrates plugins
for plugin in [net_plugin, netcfg_plugin] {
    // 1. Create checkpoint (rollback point)
    checkpoint = plugin.create_checkpoint().await
    
    // 2. Calculate diff
    diff = plugin.calculate_diff(current, desired).await
    
    // 3. Apply changes
    result = plugin.apply_state(diff).await
    
    // 4. Verify success
    if !plugin.verify_state(desired).await {
        plugin.rollback(checkpoint).await  // Undo everything!
    }
}
```

**Why Plugins?**
- **Separation of concerns**: Network vs configuration vs storage
- **Independent testing**: Each plugin tested in isolation
- **Extensibility**: Add new plugins without touching core
- **Atomic units**: Each plugin manages its own state atomically

#### 3. **Net Plugin ↔ systemd-networkd**

```rust
// Net Plugin creates systemd-networkd config files
create_file("/etc/systemd/network/ovsbr0.netdev", "[NetDev]\nName=ovsbr0\nKind=ovs-bridge\n")
create_file("/etc/systemd/network/ovsbr0.network", "[Match]\nName=ovsbr0\n[Network]\nDHCP=yes\n")
        ↓
// Reload systemd-networkd
systemctl reload systemd-networkd
        ↓
// systemd-networkd applies changes
networkctl reconfigure ovsbr0
```

**Why systemd-networkd?**
- **Declarative**: Configuration files, not commands
- **Integrated**: Part of systemd ecosystem
- **Reliable**: Well-tested, production-grade
- **Atomic**: Uses network checkpoints internally

#### 4. **systemd-networkd ↔ OVS**

```bash
# systemd-networkd creates OVS bridge
# /etc/systemd/network/ovsbr0.netdev:
[NetDev]
Name=ovsbr0
Kind=ovs-bridge
        ↓
# systemd-networkd talks to OVS via ovs-vsctl
ovs-vsctl add-br ovsbr0
        ↓
# OVS creates bridge in kernel
ovs-vswitchd creates bridge in datapath
```

**Why OVS?**
- **Production-grade virtual switching**: Used in OpenStack, Kubernetes
- **Advanced features**: VLANs, tunnels, flow rules, QoS
- **OpenFlow support**: SDN capabilities
- **Performance**: Kernel datapath for line-rate switching

#### 5. **Blockchain Ledger ↔ Everything**

```rust
// Every state change is logged
ledger.append("apply_state", json!({
    "plugin": "net",
    "timestamp": "2025-10-14T12:00:00Z",
    "user": "admin",
    "host": "server01",
    "changes": diff,
    "result": "success"
}))
        ↓
// Creates immutable chain
Block N-1 ← [SHA-256] ← Block N ← [SHA-256] ← Block N+1
```

**Why Blockchain?**
- **Immutable audit trail**: Cannot be modified or deleted
- **Cryptographic integrity**: Tampering detected immediately
- **Compliance**: Meet regulatory audit requirements
- **Forensics**: Understand exactly what changed when

---

## Why State Management is Critical

### The Problem with Imperative Network Management

Traditional network management:

```bash
# Traditional approach (DANGEROUS!)
ovs-vsctl add-br ovsbr0
ovs-vsctl add-port ovsbr0 eth0
ip addr add 192.168.1.100/24 dev ovsbr0
ip link set ovsbr0 up
ip route add default via 192.168.1.1

# What if any step fails? 😱
# - Partial configuration applied
# - Network broken
# - No automatic rollback
# - Manual recovery required
# - Downtime!
```

**Problems:**
1. **No atomicity**: Changes applied one-by-one, failures leave system in broken state
2. **No verification**: Hope configuration is correct
3. **No rollback**: Manual recovery on failure
4. **No audit trail**: Unknown who changed what when
5. **Race conditions**: Multiple admins making changes simultaneously

### The Solution: Declarative State Management

```yaml
# Desired state (SAFE!)
version: 1
plugins:
  net:
    interfaces:
      - name: ovsbr0
        type: ovs-bridge
        ports:
          - eth0
        ipv4:
          enabled: true
          address:
            - ip: 192.168.1.100
              prefix: 24
          gateway: 192.168.1.1
```

```bash
# Apply state atomically
sudo ovs-port-agent apply-state network.yaml
```

**What happens behind the scenes:**

```
Phase 1: CHECKPOINT CREATION
├─ Save current network state
├─ Create systemd-networkd checkpoint
└─ Save OVS bridge state
   ✅ Rollback point established

Phase 2: DIFF CALCULATION
├─ Query current state
├─ Compare with desired state
└─ Calculate required actions
   ✅ Know exactly what will change

Phase 3: ATOMIC APPLICATION
├─ Action 1: Create OVS bridge
│  ✅ Success
├─ Action 2: Add port to bridge
│  ✅ Success
├─ Action 3: Configure IP address
│  ✅ Success
└─ Action 4: Set default gateway
   ✅ Success

Phase 4: VERIFICATION
├─ Query new state
├─ Compare with desired state
└─ Verify all changes applied
   ✅ State matches desired

Phase 5: BLOCKCHAIN LOGGING
├─ Hash operation details
├─ Link to previous block
└─ Write to immutable ledger
   ✅ Audit trail created

Result: SUCCESS ✅
Network reconfigured with ZERO DOWNTIME
```

**If any step fails:**

```
Phase 3: ATOMIC APPLICATION
├─ Action 1: Create OVS bridge
│  ✅ Success
├─ Action 2: Add port to bridge
│  ✅ Success
├─ Action 3: Configure IP address
│  ❌ FAILED!
└─ ROLLBACK TRIGGERED

ROLLBACK:
├─ Restore checkpoint from Phase 1
├─ Undo Action 2 (remove port)
├─ Undo Action 1 (delete bridge)
└─ Verify original state restored
   ✅ Network restored to original state

Result: FAILURE (but safe) ✅
Network unchanged - no downtime!
```

### Key Benefits of State Management

#### 1. **Atomicity (All-or-Nothing)**

```
Traditional:                   State-Managed:
┌──────────────┐              ┌──────────────┐
│ Change 1 ✅  │              │  Change 1    │
│ Change 2 ✅  │              │  Change 2    │
│ Change 3 ❌  │              │  Change 3    │
│ Change 4 ⚠️  │              │  Change 4    │
├──────────────┤              ├──────────────┤
│ BROKEN       │              │ ✅ All       │
│ STATE! 😱    │              │ OR           │
│              │              │ ❌ None      │
└──────────────┘              └──────────────┘
```

#### 2. **Idempotency (Safe to Repeat)**

```bash
# Apply same state 100 times - same result
for i in {1..100}; do
    ovs-port-agent apply-state network.yaml
done

# Result: Network in desired state, no errors
# System detects: current == desired, no changes needed
```

#### 3. **Predictability (Know Before You Go)**

```bash
# See what WILL change before applying
ovs-port-agent show-diff network.yaml

# Output:
# {
#   "plugin": "net",
#   "actions": [
#     {
#       "type": "Create",
#       "resource": "ovsbr0",
#       "config": {...}
#     },
#     {
#       "type": "Modify",
#       "resource": "eth0",
#       "changes": {"controller": "ovsbr0"}
#     }
#   ]
# }
```

#### 4. **Auditability (Complete History)**

```bash
# Who did what when
dbus-send --system --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blockchain_stats

# Output:
# {
#   "total_blocks": 1247,
#   "earliest": "2025-01-01T00:00:00Z",
#   "latest": "2025-10-14T12:00:00Z",
#   "categories": {
#     "network": 523,
#     "netcfg": 187,
#     "user": 42
#   }
# }
```

#### 5. **Safety (Automatic Rollback)**

```
Error detected → Automatic rollback → Original state restored
     ↓                    ↓                      ↓
  ❌ Failure        📸 Checkpoint          ✅ Zero downtime
```

---

## Component Relationships

### State Flow Diagram

```
┌────────────────────────────────────────────────────────────────┐
│                     DESIRED STATE (YAML)                       │
│                                                                 │
│  version: 1                                                    │
│  plugins:                                                      │
│    net:                                                        │
│      interfaces: [...]                                        │
└────────────────────────────────────────────────────────────────┘
                           ↓
                ┌──────────────────────┐
                │  LOAD & VALIDATE     │
                │  - Parse YAML        │
                │  - Validate schema   │
                │  - Check plugins     │
                └──────────────────────┘
                           ↓
┌────────────────────────────────────────────────────────────────┐
│                    CURRENT STATE (LIVE)                        │
│                                                                 │
│  Query from:                                                   │
│  - systemd-networkd (D-Bus)                                    │
│  - OVS database (ovs-vsctl)                                    │
│  - Linux kernel (netlink)                                      │
└────────────────────────────────────────────────────────────────┘
                           ↓
                ┌──────────────────────┐
                │   DIFF CALCULATION   │
                │   - Compare states   │
                │   - Generate actions │
                │   - Order by deps    │
                └──────────────────────┘
                           ↓
              ┌────────────────────────────┐
              │  CHECKPOINT CREATION       │
              │  - Save current state      │
              │  - Mark rollback point     │
              │  - Generate checkpoint ID  │
              └────────────────────────────┘
                           ↓
           ┌───────────────────────────────────┐
           │     ATOMIC APPLICATION            │
           │                                   │
           │  for each action:                 │
           │    execute() → verify()           │
           │    if failed: rollback_all()      │
           └───────────────────────────────────┘
                           ↓
                  ┌────────────────┐
                  │  VERIFICATION  │
                  │  - Query state │
                  │  - Compare     │
                  │  - Validate    │
                  └────────────────┘
                           ↓
              ┌──────────────────────────┐
              │  BLOCKCHAIN LOGGING      │
              │  - Hash operation        │
              │  - Link to prev block    │
              │  - Write to ledger       │
              └──────────────────────────┘
                           ↓
              ┌──────────────────────────┐
              │     SUCCESS / FAILURE    │
              │  - Return to caller      │
              │  - Emit D-Bus signal     │
              │  - Log to journal        │
              └──────────────────────────┘
```

### Plugin Architecture

#### Net Plugin (Infrastructure - Set in Stone)

**Responsibility:** Core network infrastructure that rarely changes

```rust
Net Plugin manages:
├─ OVS Bridges (ovsbr0, ovsbr1)
│  ├─ Bridge creation/deletion
│  ├─ Port membership
│  └─ STP/RSTP settings
│
├─ Physical Interfaces (eth0, enp2s0)
│  ├─ Enslavement to bridges
│  ├─ MTU settings
│  └─ Link state
│
├─ IP Addresses
│  ├─ IPv4 configuration
│  ├─ IPv6 configuration
│  └─ DHCP vs static
│
└─ Gateway Configuration
   ├─ Default routes
   └─ Metric settings
```

**State File Example:**

```yaml
version: 1
plugins:
  net:
    interfaces:
      # OVS Bridge with static IP
      - name: ovsbr0
        type: ovs-bridge
        ports:
          - eth0  # Physical uplink
        ipv4:
          enabled: true
          dhcp: false
          address:
            - ip: 192.168.1.100
              prefix: 24
          gateway: 192.168.1.1
          dns:
            - 8.8.8.8
            - 8.8.4.4
      
      # Physical interface (enslaved)
      - name: eth0
        type: ethernet
        controller: ovsbr0  # Enslaved to bridge
        ipv4:
          enabled: false  # No IP on enslaved interface
      
      # Container bridge (no uplink)
      - name: ovsbr1
        type: ovs-bridge
        ipv4:
          enabled: true
          dhcp: false
          address:
            - ip: 10.0.100.1
              prefix: 24
```

**Backend Operations:**

```bash
# Net Plugin creates systemd-networkd files:

# /etc/systemd/network/ovsbr0.netdev
[NetDev]
Name=ovsbr0
Kind=ovs-bridge

# /etc/systemd/network/ovsbr0.network
[Match]
Name=ovsbr0

[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
DNS=8.8.8.8
DNS=8.8.4.4

# /etc/systemd/network/eth0.network
[Match]
Name=eth0

[Network]
Bridge=ovsbr0
LinkLocalAddressing=no
DHCP=no

# Then:
systemctl reload systemd-networkd
networkctl reconfigure ovsbr0
```

#### Netcfg Plugin (Configuration - Tunable)

**Responsibility:** Network configuration that changes frequently

```rust
Netcfg Plugin manages:
├─ Routing Tables
│  ├─ Static routes
│  ├─ Policy routing
│  └─ Route metrics
│
├─ DNS Configuration
│  ├─ Hostname
│  ├─ Search domains
│  └─ DNS servers (additional)
│
├─ OVS Flow Rules
│  ├─ Flow table entries
│  ├─ Priority settings
│  └─ Actions
│
└─ Firewall Rules (planned)
   ├─ iptables rules
   └─ nftables rules
```

**State File Example:**

```yaml
version: 1
plugins:
  netcfg:
    routing:
      # Static route for VPN network
      - destination: 10.0.0.0/8
        gateway: 192.168.1.254
        interface: ovsbr0
        metric: 100
    
    dns:
      hostname: netmaker-server
      search_domains:
        - vpn.internal
        - example.com
    
    ovs_flows:
      # Allow VPN traffic
      - bridge: ovsbr0
        priority: 200
        match_rule: "ip,nw_dst=10.0.0.0/8"
        actions: "normal"
      
      # Drop specific traffic
      - bridge: ovsbr0
        priority: 100
        match_rule: "ip,nw_src=192.168.100.0/24"
        actions: "drop"
```

**Backend Operations:**

```bash
# Netcfg Plugin uses various tools:

# Routing
ip route add 10.0.0.0/8 via 192.168.1.254 dev ovsbr0 metric 100

# DNS (via systemd-resolved)
hostnamectl set-hostname netmaker-server
echo "search_domains=vpn.internal" >> /etc/systemd/resolved.conf
systemctl restart systemd-resolved

# OVS flows
ovs-ofctl add-flow ovsbr0 "priority=200,ip,nw_dst=10.0.0.0/8,actions=normal"
ovs-ofctl add-flow ovsbr0 "priority=100,ip,nw_src=192.168.100.0/24,actions=drop"
```

### State Transitions

```
STATE LIFECYCLE:

┌─────────────┐
│  DESIRED    │  ← User defines this (YAML file)
│   STATE     │
└─────────────┘
      ↓
┌─────────────┐
│  CURRENT    │  ← System queries this (live)
│   STATE     │
└─────────────┘
      ↓
┌─────────────┐
│    DIFF     │  ← System calculates this
│  (Actions)  │
└─────────────┘
      ↓
┌─────────────┐
│ CHECKPOINT  │  ← System creates rollback point
└─────────────┘
      ↓
┌─────────────┐
│   APPLY     │  ← System executes actions
│  (Atomic)   │
└─────────────┘
      ↓
    ┌───┴───┐
    │Success│Failure
    ↓       ↓
┌─────────┐ ┌─────────┐
│ VERIFY  │ │ROLLBACK │
│ STATE   │ │TO CHKPT │
└─────────┘ └─────────┘
    ↓            ↓
┌─────────────────────┐
│  BLOCKCHAIN LOG     │
│  (Immutable Record) │
└─────────────────────┘
```

---

## State Lifecycle

### 1. State Definition (YAML)

**User creates desired state:**

```yaml
# /opt/network-config/production.yaml
version: 1
plugins:
  net:
    interfaces:
      - name: ovsbr0
        type: ovs-bridge
        ports: [eth0]
        ipv4:
          enabled: true
          dhcp: true
```

### 2. State Loading

```rust
// System loads and validates state
let desired_state = state_manager
    .load_desired_state("/opt/network-config/production.yaml")
    .await?;

// Validates:
// ✅ YAML syntax correct
// ✅ Schema version supported
// ✅ Plugins exist and registered
// ✅ Configuration valid
```

### 3. Current State Query

```rust
// Query live system state via plugins
let current_state = state_manager
    .query_current_state()
    .await?;

// Queries via:
// - D-Bus (systemd-networkd state)
// - OVS database (ovs-vsctl list)
// - Netlink (kernel state)
// - Filesystem (config files)
```

**D-Bus Query Example:**

```bash
# systemd-networkd D-Bus introspection
busctl call org.freedesktop.network1 \
  /org/freedesktop/network1 \
  org.freedesktop.network1.Manager \
  ListLinks

# Returns:
# [(2, "eth0", "/org/freedesktop/network1/link/_32")]
# [(3, "ovsbr0", "/org/freedesktop/network1/link/_33")]
```

### 4. Diff Calculation

```rust
// Calculate differences between current and desired
let diffs = state_manager
    .calculate_all_diffs(&desired_state)
    .await?;

// Example diff:
StateDiff {
    plugin: "net",
    actions: vec![
        StateAction::Create {
            resource: "ovsbr0",
            config: {...}
        },
        StateAction::Modify {
            resource: "eth0",
            changes: {"controller": "ovsbr0"}
        }
    ],
    metadata: DiffMetadata {
        timestamp: 1728907200,
        current_hash: "abc123...",
        desired_hash: "def456..."
    }
}
```

### 5. Checkpoint Creation

```rust
// Create rollback point before changes
let checkpoint = plugin.create_checkpoint().await?;

// Saves:
// - Current network state
// - systemd-networkd configuration files
// - OVS database snapshot
// - Timestamp and ID
```

**Checkpoint Structure:**

```json
{
  "id": "chkpt-1728907200-abc123",
  "plugin": "net",
  "timestamp": 1728907200,
  "state_snapshot": {
    "interfaces": [...],
    "routes": [...],
    "ovs_bridges": [...]
  },
  "backend_checkpoint": {
    "systemd_files": {
      "/etc/systemd/network/eth0.network": "...",
      "/etc/systemd/network/ovsbr0.netdev": "..."
    },
    "ovs_database": "..."
  }
}
```

### 6. Atomic Application

```rust
// Apply all changes atomically
for action in diff.actions {
    match action {
        StateAction::Create { resource, config } => {
            plugin.create_resource(resource, config).await?;
        }
        StateAction::Modify { resource, changes } => {
            plugin.modify_resource(resource, changes).await?;
        }
        StateAction::Delete { resource } => {
            plugin.delete_resource(resource).await?;
        }
    }
    
    // CRITICAL: If ANY action fails, rollback ALL
    if result.is_err() {
        rollback_all_plugins(&checkpoints).await?;
        return Err("Apply failed, rolled back");
    }
}
```

### 7. State Verification

```rust
// Verify state matches desired
let verified = state_manager
    .verify_all_states(&desired_state)
    .await?;

if !verified {
    // Verification failed - rollback!
    rollback_all_plugins(&checkpoints).await?;
    return Err("Verification failed");
}
```

**Verification Checks:**

```bash
# Net Plugin verification
✅ Bridge exists: ovsbr0
✅ Port attached: eth0 → ovsbr0
✅ IP address configured: 192.168.1.100/24
✅ Gateway set: 192.168.1.1
✅ Interface state: UP
✅ Routing table correct
```

### 8. Blockchain Logging

```rust
// Log operation to immutable ledger
ledger.append("apply_state", json!({
    "plugin": "net",
    "user": "admin",
    "host": hostname(),
    "timestamp": chrono::Utc::now(),
    "desired_state_hash": hash(&desired_state),
    "actions": diff.actions,
    "result": "success",
    "checkpoint_id": checkpoint.id
}));

// Creates blockchain entry with SHA-256 hash
```

**Blockchain Entry:**

```json
{
  "height": 1247,
  "prev_hash": "f7a2b3c4...",
  "timestamp": "2025-10-14T12:00:00Z",
  "category": "apply_state",
  "action": "net",
  "metadata": {
    "user": "admin",
    "host": "server01",
    "plugin": "net"
  },
  "data": {
    "desired_state_hash": "abc123...",
    "actions": [...],
    "result": "success"
  },
  "hash": "d1e2f3a4..."
}
```

---

## Deployment Procedures

### Prerequisites

```bash
# 1. Install required packages
apt-get update
apt-get install -y \
  openvswitch-switch \
  systemd \
  dbus \
  build-essential

# 2. Verify systemd-networkd is available
systemctl status systemd-networkd

# 3. Verify OVS is running
systemctl status openvswitch-switch
ovs-vsctl --version

# 4. Disable conflicting network managers
systemctl disable NetworkManager
systemctl stop NetworkManager
systemctl mask NetworkManager
```

### Installation

#### Option 1: From Source

```bash
# 1. Clone repository
git clone https://github.com/yourorg/nm-monitor.git
cd nm-monitor

# 2. Build with Cargo
cargo build --release

# 3. Install binary
sudo cp target/release/ovs-port-agent /usr/local/bin/

# 4. Install systemd service
sudo cp systemd/ovs-port-agent.service /etc/systemd/system/
sudo systemctl daemon-reload

# 5. Create configuration directory
sudo mkdir -p /etc/ovs-port-agent
sudo cp config/config.toml.example /etc/ovs-port-agent/config.toml

# 6. Create ledger directory
sudo mkdir -p /var/lib/ovs-port-agent
```

#### Option 2: Using Install Script

```bash
# Basic installation
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink eth0 \
  --system

# With secondary bridge (for containers)
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink eth0 \
  --with-ovsbr1 \
  --system

# Proxmox deployment
sudo ./scripts/install.sh \
  --bridge ovsbr0 \
  --uplink vmbr0 \
  --with-ovsbr1 \
  --system \
  --purge-bridges
```

### Initial Configuration

#### 1. Edit Configuration File

```toml
# /etc/ovs-port-agent/config.toml

# Primary OVS bridge
bridge_name = "ovsbr0"

# Interfaces file path (for Proxmox)
interfaces_path = "/etc/network/interfaces"

# Blockchain ledger path
ledger_path = "/var/lib/ovs-port-agent/ledger.jsonl"

# Interface naming
naming_template = "vi{vmid}"
enable_rename = false

# Managed block tag
managed_block_tag = "ovs-port-agent"
```

#### 2. Create Initial Network State

```bash
# Create state directory
sudo mkdir -p /opt/network-config

# Create initial state file
sudo nano /opt/network-config/initial-state.yaml
```

```yaml
# /opt/network-config/initial-state.yaml
version: 1
plugins:
  net:
    interfaces:
      # Primary bridge with uplink
      - name: ovsbr0
        type: ovs-bridge
        ports:
          - eth0
        ipv4:
          enabled: true
          dhcp: true  # Or configure static IP
      
      # Physical uplink (enslaved)
      - name: eth0
        type: ethernet
        controller: ovsbr0
        ipv4:
          enabled: false
```

#### 3. Apply Initial State

```bash
# IMPORTANT: Review what will change first
sudo ovs-port-agent show-diff /opt/network-config/initial-state.yaml

# If output looks correct, apply
sudo ovs-port-agent apply-state /opt/network-config/initial-state.yaml
```

#### 4. Enable and Start Service

```bash
# Enable service at boot
sudo systemctl enable ovs-port-agent

# Start service
sudo systemctl start ovs-port-agent

# Verify status
sudo systemctl status ovs-port-agent

# Check logs
sudo journalctl -u ovs-port-agent -f
```

### Verification

```bash
# 1. Verify D-Bus interface is available
busctl list | grep dev.ovs.PortAgent1

# 2. Ping the service
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.ping

# Should return: string "pong"

# 3. Query current network state
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.GetSystemNetworkState | head -50

# 4. Verify blockchain is working
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blockchain_stats

# 5. Verify network connectivity
ping -c 4 8.8.8.8

# 6. Check OVS bridges
ovs-vsctl show

# 7. Check systemd-networkd
networkctl list
networkctl status ovsbr0
```

---

## Operational Commands

### Daily Operations

#### Query System State

```bash
# Get complete system state
sudo ovs-port-agent query-state

# Get specific plugin state
sudo ovs-port-agent query-state --plugin net
sudo ovs-port-agent query-state --plugin netcfg

# Via D-Bus
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.QueryState \
  string:"net"
```

#### Apply Configuration Changes

```bash
# 1. Show what will change (dry-run)
sudo ovs-port-agent show-diff /opt/network-config/new-state.yaml

# Example output:
# {
#   "plugin": "net",
#   "actions": [
#     {
#       "type": "Modify",
#       "resource": "ovsbr0",
#       "changes": {"ipv4.address": "192.168.1.200/24"}
#     }
#   ]
# }

# 2. Apply changes
sudo ovs-port-agent apply-state /opt/network-config/new-state.yaml

# 3. Verify success
sudo ovs-port-agent query-state --plugin net
```

#### Blockchain Operations

```bash
# Get blockchain statistics
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blockchain_stats

# Get blocks by category
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blocks_by_category \
  string:"network"

# Verify blockchain integrity
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.verify_blockchain_integrity

# Get specific block by hash
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_block_by_hash \
  string:"abc123..."

# Manually inspect ledger
sudo tail -f /var/lib/ovs-port-agent/ledger.jsonl
```

### Port Management

```bash
# List OVS ports
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.list_ports

# Add port
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.add_port \
  string:"veth123"

# Remove port
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.del_port \
  string:"veth123"

# Create container interface (with VMID)
sudo ovs-port-agent create-interface \
  veth-123-eth0 \
  container-123 \
  100

# Remove container interface
sudo ovs-port-agent remove-interface vi100
```

### System Introspection

```bash
# Comprehensive systemd-networkd introspection
sudo ovs-port-agent introspect-systemd

# Via D-Bus
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.IntrospectSystemdNetworkd
```

### Monitoring

```bash
# Real-time logs
sudo journalctl -u ovs-port-agent -f

# Recent errors
sudo journalctl -u ovs-port-agent -p err -n 50

# Monitor D-Bus events
dbus-monitor --system \
  "type='signal',sender='dev.ovs.PortAgent1'"

# Monitor systemd-networkd
journalctl -u systemd-networkd -f

# Monitor OVS
journalctl -u openvswitch-switch -f
```

---

## Troubleshooting

### Common Issues

#### Issue 1: State Apply Fails

**Symptom:**
```bash
sudo ovs-port-agent apply-state network.yaml
Error: State apply failed
```

**Diagnosis:**
```bash
# 1. Check logs
sudo journalctl -u ovs-port-agent -n 100

# 2. Verify state file syntax
cat network.yaml

# 3. Check current state
sudo ovs-port-agent query-state

# 4. Show diff
sudo ovs-port-agent show-diff network.yaml

# 5. Check blockchain for recent operations
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blocks_by_category \
  string:"network" | tail -50
```

**Solutions:**

1. **Invalid YAML syntax:**
   ```bash
   # Validate YAML
   python3 -c "import yaml; yaml.safe_load(open('network.yaml'))"
   ```

2. **Plugin not registered:**
   ```bash
   # Check service is running
   systemctl status ovs-port-agent
   # Restart if needed
   systemctl restart ovs-port-agent
   ```

3. **OVS not available:**
   ```bash
   systemctl status openvswitch-switch
   ovs-vsctl show
   ```

4. **systemd-networkd not running:**
   ```bash
   systemctl status systemd-networkd
   systemctl start systemd-networkd
   ```

#### Issue 2: Network Connectivity Lost

**Symptom:**
Network unreachable after applying state

**Immediate Recovery:**
```bash
# System should have auto-rolled back
# Verify current state
ip addr show
ip route show

# Check blockchain for rollback entry
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blocks_by_category \
  string:"rollback" | tail -20

# If auto-rollback didn't work, manual recovery:
# 1. Stop ovs-port-agent
systemctl stop ovs-port-agent

# 2. Restore original network config
cp /etc/network/interfaces.backup /etc/network/interfaces
systemctl restart networking

# 3. Check connectivity
ping -c 4 8.8.8.8
```

#### Issue 3: D-Bus Interface Not Available

**Symptom:**
```bash
busctl list | grep dev.ovs.PortAgent1
# (no output)
```

**Solution:**
```bash
# 1. Check service status
systemctl status ovs-port-agent

# 2. Check D-Bus logs
journalctl -u dbus -n 50

# 3. Verify D-Bus policy
cat /usr/share/dbus-1/system.d/dev.ovs.PortAgent1.conf

# 4. Restart service
systemctl restart ovs-port-agent

# 5. Verify D-Bus connection
busctl list | grep ovs
```

#### Issue 4: Blockchain Integrity Failed

**Symptom:**
```bash
dbus-send ... verify_blockchain_integrity
# Returns: "INVALID"
```

**Diagnosis:**
```bash
# Check ledger file
sudo cat /var/lib/ovs-port-agent/ledger.jsonl | tail -20

# Get blockchain stats
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blockchain_stats
```

**Solution:**
```bash
# Blockchain corruption is serious - indicates tampering
# DO NOT delete the ledger file - it's evidence!

# 1. Backup corrupted ledger
sudo cp /var/lib/ovs-port-agent/ledger.jsonl \
       /var/lib/ovs-port-agent/ledger.jsonl.corrupted.$(date +%s)

# 2. Create new ledger
sudo rm /var/lib/ovs-port-agent/ledger.jsonl
sudo systemctl restart ovs-port-agent

# 3. Report to security team
# Investigate: who modified the ledger file?
```

### Debug Mode

```bash
# Enable debug logging
sudo systemctl edit ovs-port-agent

# Add:
[Service]
Environment="RUST_LOG=debug"
Environment="RUST_BACKTRACE=1"

# Restart
sudo systemctl restart ovs-port-agent

# Watch debug logs
sudo journalctl -u ovs-port-agent -f
```

---

## Advanced Topics

### Custom State Plugins

Create your own plugin to manage additional system state:

```rust
// my_plugin.rs
use crate::state::plugin::*;
use async_trait::async_trait;

pub struct MyPlugin;

#[async_trait]
impl StatePlugin for MyPlugin {
    fn name(&self) -> &str { "myplugin" }
    fn version(&self) -> &str { "1.0.0" }
    
    async fn query_current_state(&self) -> Result<Value> {
        // Query your system component
        Ok(json!({"status": "active"}))
    }
    
    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        // Calculate required actions
        let actions = vec![
            StateAction::Create {
                resource: "myresource".to_string(),
                config: desired.clone()
            }
        ];
        
        Ok(StateDiff {
            plugin: "myplugin".to_string(),
            actions,
            metadata: DiffMetadata { /* ... */ }
        })
    }
    
    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        // Apply changes
        for action in &diff.actions {
            // Execute action
        }
        
        Ok(ApplyResult {
            success: true,
            changes_applied: vec!["myresource".to_string()],
            errors: vec![],
            checkpoint: None
        })
    }
    
    // Implement other required methods...
}
```

Register your plugin:

```rust
// main.rs
state_manager.register_plugin(Box::new(MyPlugin)).await;
```

### Integration with External Systems

#### Ansible Integration

```yaml
# playbook.yml
---
- name: Configure network with ovs-port-agent
  hosts: servers
  tasks:
    - name: Copy network state file
      copy:
        src: files/network-state.yaml
        dest: /opt/network-config/current.yaml
    
    - name: Apply network state
      command: ovs-port-agent apply-state /opt/network-config/current.yaml
      register: apply_result
    
    - name: Verify blockchain integrity
      shell: |
        dbus-send --system --print-reply \
          --dest=dev.ovs.PortAgent1 \
          /dev/ovs/PortAgent1 \
          dev.ovs.PortAgent1.verify_blockchain_integrity
      register: blockchain_check
      changed_when: false
    
    - name: Assert blockchain valid
      assert:
        that: "'VALID' in blockchain_check.stdout"
        fail_msg: "Blockchain integrity check failed!"
```

#### Python API Client

```python
#!/usr/bin/env python3
import dbus

# Connect to system bus
bus = dbus.SystemBus()

# Get service proxy
proxy = bus.get_object('dev.ovs.PortAgent1', '/dev/ovs/PortAgent1')
interface = dbus.Interface(proxy, 'dev.ovs.PortAgent1')

# Ping
result = interface.ping()
print(f"Ping: {result}")

# Query state
state = interface.QueryState("net")
print(f"Network state: {state}")

# Get blockchain stats
stats = interface.get_blockchain_stats()
print(f"Blockchain: {stats}")
```

#### Prometheus Monitoring

```bash
# Export blockchain stats to Prometheus
#!/bin/bash
while true; do
  stats=$(dbus-send --system --print-reply \
    --dest=dev.ovs.PortAgent1 \
    /dev/ovs/PortAgent1 \
    dev.ovs.PortAgent1.get_blockchain_stats)
  
  # Parse and expose metrics
  echo "ovs_port_agent_blockchain_blocks{} $(echo "$stats" | jq '.total_blocks')"
  
  sleep 30
done > /var/lib/prometheus/node-exporter/ovs-port-agent.prom
```

### High Availability Setup

```yaml
# HA configuration with keepalived
# /opt/network-config/ha-state.yaml
version: 1
plugins:
  net:
    interfaces:
      - name: ovsbr0
        type: ovs-bridge
        ports: [eth0]
        ipv4:
          enabled: true
          dhcp: false
          address:
            - ip: 192.168.1.100  # Primary
              prefix: 24
            - ip: 192.168.1.10   # VIP
              prefix: 24
          gateway: 192.168.1.1
```

---

## Summary

### Key Takeaways

1. **State Management is Critical** - Provides atomicity, rollback, and verification
2. **D-Bus Enables Integration** - System-wide RPC interface for all components
3. **systemd-networkd + OVS = Declarative Networking** - Configuration files, not commands
4. **Blockchain Provides Accountability** - Immutable audit trail of all changes
5. **Plugins Enable Extensibility** - Easy to add new state management capabilities

### Quick Reference

| Operation | Command |
|-----------|---------|
| **Apply State** | `ovs-port-agent apply-state file.yaml` |
| **Query State** | `ovs-port-agent query-state` |
| **Show Diff** | `ovs-port-agent show-diff file.yaml` |
| **Blockchain Stats** | D-Bus: `get_blockchain_stats` |
| **Verify Integrity** | D-Bus: `verify_blockchain_integrity` |
| **List Ports** | D-Bus: `list_ports` |
| **Check Logs** | `journalctl -u ovs-port-agent -f` |

### Support

- **Documentation**: `/git/nm-monitor/docs/`
- **Examples**: `/git/nm-monitor/config/examples/`
- **Logs**: `journalctl -u ovs-port-agent`
- **Blockchain**: `/var/lib/ovs-port-agent/ledger.jsonl`

---

**Built for operational excellence, accountability, and zero-downtime deployments.** 🚀
