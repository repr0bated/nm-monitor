# Universal Declarative State Manager Architecture

## Overview

A **pluggable, declarative state management system** for managing ANY aspect of system configuration - network, filesystems, users, config files, sessions, storage, and more.

Inspired by nmstate but generalized to handle all system state through a unified plugin architecture.

## Philosophy

**"Declare what you want, plugins figure out how to get there"**

- Declarative YAML/JSON schema for desired state
- Plugins implement domain-specific state management
- Automatic diff calculation and application
- Blockchain audit trail for all changes
- Atomic operations with rollback capability

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│           Declarative State Manager (Core)              │
│  - Parse YAML/JSON desired state                       │
│  - Route to appropriate plugins                        │
│  - Orchestrate apply/verify/rollback                   │
│  - Coordinate atomic transactions                      │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                 State Plugin Trait                      │
│  - query_current_state()  → JSON                       │
│  - calculate_diff(current, desired) → Diff             │
│  - apply_state(diff) → Result                          │
│  - verify_state(desired) → bool                        │
│  - rollback(checkpoint) → Result                       │
└─────────────────────────────────────────────────────────┘
                          ↓
┌──────────┬──────────┬──────────┬──────────┬──────────┐
│ Network  │Filesystem│  User    │  Config  │ Storage  │
│ Plugin   │ Plugin   │  Plugin  │  Plugin  │  Plugin  │
└──────────┴──────────┴──────────┴──────────┴──────────┘
     ↓           ↓          ↓          ↓          ↓
┌──────────┬──────────┬──────────┬──────────┬──────────┐
│systemd-  │  btrfs   │ systemd- │   toml   │   lvm    │
│networkd  │   zfs    │  loginctl│   yaml   │  partitions│
│  +ovs    │filesystem│   users  │  /etc/*  │   mounts │
└──────────┴──────────┴──────────┴──────────┴──────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│              Blockchain Ledger Integration              │
│  - Record all state changes with SHA-256 chain         │
│  - Immutable audit trail                               │
│  - Category: network, filesystem, user, config, etc.   │
└─────────────────────────────────────────────────────────┘
```

## Core State Schema

### Unified State Format

```yaml
# /etc/ovs-port-agent/state.yaml
version: 1

# Each top-level key maps to a plugin
network:
  interfaces:
    - name: enp2s0
      type: ethernet
      ipv4:
        enabled: true
        address:
          - ip: 192.168.1.10
            prefix: 24
        gateway: 192.168.1.1
        dns: [1.1.1.1, 8.8.8.8]

    - name: ovsbr0
      type: ovs-bridge
      ports:
        - enp2s0
        - veth-vm100
      ipv4:
        enabled: true
        dhcp: false

filesystem:
  mounts:
    - source: UUID=abc123
      target: /data
      fstype: btrfs
      options: [compress=zstd, subvol=@data]
      state: mounted

  snapshots:
    - path: /data
      schedule: hourly
      retention: 7

users:
  accounts:
    - name: jeremy
      uid: 1000
      groups: [wheel, docker, libvirt]
      shell: /bin/bash
      state: present

  sessions:
    - user: jeremy
      type: x11
      display: ":0"

config:
  files:
    - path: /etc/ssh/sshd_config
      template: templates/sshd_config.j2
      mode: "0644"
      owner: root
      checksum: sha256:abc123...

  services:
    - name: sshd
      enabled: true
      state: running

storage:
  volumes:
    - name: data-pool
      type: lvm
      size: 500G
      state: present

  disks:
    - device: /dev/sda
      partition_table: gpt
      partitions:
        - number: 1
          size: 1G
          type: efi
```

## Plugin Trait Definition

```rust
// src/state/plugin.rs

use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;

/// Core trait that all state management plugins must implement
#[async_trait]
pub trait StatePlugin: Send + Sync {
    /// Plugin identifier (e.g., "network", "filesystem", "user")
    fn name(&self) -> &str;

    /// Plugin version for compatibility
    fn version(&self) -> &str;

    /// Query current system state in this domain
    async fn query_current_state(&self) -> Result<Value>;

    /// Calculate difference between current and desired state
    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff>;

    /// Apply the state changes (may be multi-step)
    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult>;

    /// Verify that current state matches desired state
    async fn verify_state(&self, desired: &Value) -> Result<bool>;

    /// Create a checkpoint for rollback capability
    async fn create_checkpoint(&self) -> Result<Checkpoint>;

    /// Rollback to a previous checkpoint
    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()>;

    /// Get plugin capabilities and limitations
    fn capabilities(&self) -> PluginCapabilities;
}

/// Represents the difference between current and desired state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub plugin: String,
    pub actions: Vec<StateAction>,
    pub metadata: DiffMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateAction {
    Create { resource: String, config: Value },
    Modify { resource: String, changes: Value },
    Delete { resource: String },
    NoOp { resource: String },
}

/// Result of applying state changes
#[derive(Debug)]
pub struct ApplyResult {
    pub success: bool,
    pub changes_applied: Vec<String>,
    pub errors: Vec<String>,
    pub checkpoint: Option<Checkpoint>,
}

/// Checkpoint for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub plugin: String,
    pub timestamp: i64,
    pub state_snapshot: Value,
    pub backend_checkpoint: Option<Value>, // Plugin-specific checkpoint data
}
```

## Plugin Implementations

### 1. Network State Plugin (systemd-networkd + OVS)

```rust
// src/state/plugins/network.rs

pub struct NetworkStatePlugin {
    networkd_client: SystemdNetworkdClient,
    ovs_manager: OvsManager,
}

#[async_trait]
impl StatePlugin for NetworkStatePlugin {
    fn name(&self) -> &str { "network" }

    async fn query_current_state(&self) -> Result<Value> {
        // Query via D-Bus introspection
        let links = self.networkd_client.get_network_state().await?;
        let ovs_bridges = self.ovs_manager.list_bridges().await?;

        Ok(json!({
            "interfaces": links,
            "bridges": ovs_bridges,
            "routes": self.get_routes().await?,
            "dns": self.get_dns().await?
        }))
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        // Write .network files to /etc/systemd/network/
        // Call networkctl reload
        // Verify connectivity
        // Record in blockchain ledger
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        // Snapshot current .network files
        // Store current systemd-networkd state
    }
}
```

### 2. Filesystem State Plugin (btrfs/zfs)

```rust
// src/state/plugins/filesystem.rs

pub struct FilesystemStatePlugin {
    filesystem_type: FsType, // btrfs, zfs, ext4, xfs
}

#[async_trait]
impl StatePlugin for FilesystemStatePlugin {
    fn name(&self) -> &str { "filesystem" }

    async fn query_current_state(&self) -> Result<Value> {
        // Query mounts from /proc/mounts + D-Bus
        // Query btrfs subvolumes
        // Query snapshots
        Ok(json!({
            "mounts": self.get_mounts().await?,
            "snapshots": self.get_snapshots().await?,
            "quotas": self.get_quotas().await?
        }))
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        // Create btrfs snapshot
        // Record filesystem state
    }
}
```

### 3. User State Plugin (systemd-logind)

```rust
// src/state/plugins/user.rs

pub struct UserStatePlugin {
    logind_client: SystemdLogindClient,
}

#[async_trait]
impl StatePlugin for UserStatePlugin {
    fn name(&self) -> &str { "user" }

    async fn query_current_state(&self) -> Result<Value> {
        // Query users from /etc/passwd + systemd-logind D-Bus
        // Query active sessions
        Ok(json!({
            "accounts": self.get_users().await?,
            "sessions": self.get_sessions().await?,
            "groups": self.get_groups().await?
        }))
    }
}
```

### 4. Config File State Plugin

```rust
// src/state/plugins/config.rs

pub struct ConfigFileStatePlugin {
    template_engine: TemplateEngine,
}

#[async_trait]
impl StatePlugin for ConfigFileStatePlugin {
    fn name(&self) -> &str { "config" }

    async fn query_current_state(&self) -> Result<Value> {
        // Read managed config files
        // Calculate checksums
        Ok(json!({
            "files": self.get_managed_files().await?,
            "services": self.get_service_states().await?
        }))
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        // Render templates
        // Write config files atomically
        // Reload/restart services
        // Verify service states
    }
}
```

### 5. Storage State Plugin (LVM/partitions)

```rust
// src/state/plugins/storage.rs

pub struct StorageStatePlugin;

#[async_trait]
impl StatePlugin for StorageStatePlugin {
    fn name(&self) -> &str { "storage" }

    async fn query_current_state(&self) -> Result<Value> {
        // Query LVM volumes
        // Query partitions
        // Query disk usage
        Ok(json!({
            "volumes": self.get_lvm_volumes().await?,
            "partitions": self.get_partitions().await?,
            "disks": self.get_disks().await?
        }))
    }
}
```

## State Manager Core

```rust
// src/state/manager.rs

pub struct StateManager {
    plugins: HashMap<String, Box<dyn StatePlugin>>,
    ledger: Arc<Ledger>,
}

impl StateManager {
    /// Register a state plugin
    pub fn register_plugin(&mut self, plugin: Box<dyn StatePlugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    /// Load desired state from YAML/JSON
    pub async fn load_desired_state(&self, path: &Path) -> Result<DesiredState> {
        let content = tokio::fs::read_to_string(path).await?;
        serde_yaml::from_str(&content)
    }

    /// Query current state across all plugins
    pub async fn query_current_state(&self) -> Result<CurrentState> {
        let mut state = HashMap::new();

        for (name, plugin) in &self.plugins {
            state.insert(name.clone(), plugin.query_current_state().await?);
        }

        Ok(CurrentState { plugins: state })
    }

    /// Apply desired state (atomic across all plugins)
    pub async fn apply_state(&self, desired: DesiredState) -> Result<ApplyReport> {
        let mut checkpoints = Vec::new();
        let mut results = Vec::new();

        // Phase 1: Create checkpoints for all affected plugins
        for (plugin_name, desired_state) in desired.plugins.iter() {
            if let Some(plugin) = self.plugins.get(plugin_name) {
                let checkpoint = plugin.create_checkpoint().await?;
                checkpoints.push((plugin_name.clone(), checkpoint));
            }
        }

        // Phase 2: Calculate diffs
        let diffs = self.calculate_all_diffs(&desired).await?;

        // Phase 3: Apply changes in dependency order
        for diff in diffs {
            let plugin = self.plugins.get(&diff.plugin).unwrap();

            match plugin.apply_state(&diff).await {
                Ok(result) => {
                    // Log to blockchain
                    self.ledger.add_block(
                        &diff.plugin,
                        "apply_state",
                        &serde_json::to_value(&result)?
                    )?;
                    results.push(result);
                }
                Err(e) => {
                    // Rollback all plugins
                    warn!("State apply failed: {}, rolling back", e);
                    self.rollback_all(&checkpoints).await?;
                    return Err(e);
                }
            }
        }

        // Phase 4: Verify all states match desired
        let verified = self.verify_all_states(&desired).await?;

        if !verified {
            warn!("State verification failed, rolling back");
            self.rollback_all(&checkpoints).await?;
            return Err(anyhow!("State verification failed"));
        }

        Ok(ApplyReport {
            success: true,
            results,
            checkpoints,
        })
    }

    /// Rollback all plugins to checkpoints
    async fn rollback_all(&self, checkpoints: &[(String, Checkpoint)]) -> Result<()> {
        for (plugin_name, checkpoint) in checkpoints.iter().rev() {
            if let Some(plugin) = self.plugins.get(plugin_name) {
                plugin.rollback(checkpoint).await?;

                // Log rollback to blockchain
                self.ledger.add_block(
                    plugin_name,
                    "rollback",
                    &serde_json::json!({"checkpoint_id": checkpoint.id})
                )?;
            }
        }
        Ok(())
    }
}
```

## D-Bus API Integration

```rust
// src/rpc.rs - Add state management methods

#[zbus::interface(name = "dev.ovs.PortAgent1")]
impl PortAgentInterface {
    /// Apply declarative state from YAML/JSON
    async fn apply_state(&self, state_yaml: String) -> zbus::fdo::Result<String> {
        let state = serde_yaml::from_str(&state_yaml)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let report = self.state_manager.apply_state(state).await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string_pretty(&report).unwrap())
    }

    /// Query current state across all plugins
    async fn query_state(&self, plugin: Option<String>) -> zbus::fdo::Result<String> {
        let state = if let Some(p) = plugin {
            self.state_manager.query_plugin_state(&p).await
        } else {
            self.state_manager.query_current_state().await
        }.map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string_pretty(&state).unwrap())
    }

    /// Show diff between current and desired state
    async fn show_diff(&self, desired_yaml: String) -> zbus::fdo::Result<String> {
        // Calculate and return human-readable diff
    }
}
```

## Usage Examples

### Example 1: Network Configuration

```yaml
# /etc/ovs-port-agent/network-state.yaml
version: 1

network:
  interfaces:
    - name: enp2s0
      type: ethernet
      controller: ovsbr0
      ipv4:
        enabled: false  # No IP on enslaved interface

    - name: ovsbr0
      type: ovs-bridge
      ports:
        - enp2s0
        - veth-vm100
      ipv4:
        enabled: true
        address:
          - ip: 192.168.1.10
            prefix: 24
        gateway: 192.168.1.1
        dns: [1.1.1.1, 8.8.8.8]
```

```bash
# Apply network state
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.apply_state \
  string:"$(cat network-state.yaml)"

# Query current network state
dbus-send --system --dest=dev.ovs.PortAgent1 --type=method_call \
  /dev/ovs/PortAgent1 dev.ovs.PortAgent1.query_state string:network
```

### Example 2: Full System State

```yaml
# /etc/ovs-port-agent/full-system-state.yaml
version: 1

network:
  interfaces:
    - name: ovsbr0
      type: ovs-bridge
      # ... network config

filesystem:
  mounts:
    - source: /dev/vg0/data
      target: /data
      fstype: btrfs
      state: mounted

  snapshots:
    - path: /data
      name: backup-$(date +%Y%m%d)
      retention: 7

users:
  accounts:
    - name: app-user
      uid: 2000
      groups: [docker]
      shell: /bin/bash
      state: present

config:
  files:
    - path: /etc/app/config.toml
      content: |
        [app]
        port = 8080
        database = "/data/db"
      mode: "0644"
      owner: app-user

  services:
    - name: app
      enabled: true
      state: running
```

```bash
# Apply full system state atomically
sudo ovs-port-agent apply-state /etc/ovs-port-agent/full-system-state.yaml

# If anything fails, everything rolls back automatically
```

## Benefits

1. **Unified Management**: Single declarative format for ALL system state
2. **Plugin Architecture**: Easy to extend to new domains (VMs, containers, packages, etc.)
3. **Atomic Operations**: All-or-nothing with automatic rollback
4. **Blockchain Audit**: Every state change tracked immutably
5. **D-Bus Integration**: System-wide access via D-Bus
6. **Idempotent**: Apply same state file repeatedly, only changes what's needed

## Implementation Phases

### Phase 1: Core Framework (Week 1-2)
- [ ] State plugin trait
- [ ] State manager core
- [ ] YAML/JSON schema parsing
- [ ] Diff calculation engine

### Phase 2: Network Plugin (Week 2-3)
- [ ] systemd-networkd backend
- [ ] OVS bridge management
- [ ] .network file generation
- [ ] State verification

### Phase 3: Additional Plugins (Week 3-4)
- [ ] Filesystem plugin (btrfs/zfs)
- [ ] Config file plugin
- [ ] User/session plugin
- [ ] Storage plugin

### Phase 4: Integration (Week 4-5)
- [ ] D-Bus API endpoints
- [ ] CLI commands
- [ ] Blockchain ledger integration
- [ ] Documentation and testing

## Next Steps

1. Create `src/state/` module structure
2. Implement core `StatePlugin` trait
3. Build `StateManager` orchestrator
4. Implement network plugin with systemd-networkd
5. Add D-Bus methods for state management
6. Integrate with existing blockchain ledger
7. Write comprehensive tests

This creates a **universal declarative infrastructure-as-code system** that goes way beyond nmstate! 🚀
