# Hash Footprint Design for FUSE Layer

## 🎯 Concept

Every network element (interface binding, bridge config, port) gets a **cryptographic hash footprint** at creation that captures its complete configuration state.

## 🔐 Benefits

### 1. **Integrity Verification**
- Detect if interface configuration was modified
- Alert on unexpected changes
- Validate system state matches expected

### 2. **Configuration Tracking**
- Track configuration drift over time
- Compare current vs. original state
- Identify when changes occurred

### 3. **Deduplication**
- Detect identical configurations
- Reuse existing bindings safely
- Optimize storage of config data

### 4. **Audit Trail**
- Cryptographic proof of configuration
- Link to blockchain ledger
- Tamper-evident history

### 5. **Content-Addressable Storage**
- Store configs by their hash
- Immutable config snapshots
- Easy retrieval by fingerprint

## 📐 Architecture

### Hash Footprint Structure

```rust
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

/// Cryptographic hash footprint for network elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashFootprint {
    /// SHA-256 hash of the complete configuration
    pub config_hash: String,
    
    /// SHA-256 hash of the runtime state
    pub state_hash: String,
    
    /// Combined fingerprint (hash of config + state)
    pub fingerprint: String,
    
    /// Timestamp when footprint was created
    pub created_at: String,
    
    /// Fields included in the hash (for verification)
    pub hashed_fields: Vec<String>,
    
    /// Algorithm used (for future flexibility)
    pub algorithm: String,
}

impl HashFootprint {
    /// Create new footprint from configuration
    pub fn from_config<T: Serialize>(config: &T) -> Result<Self> {
        let config_json = serde_json::to_string(config)?;
        let config_hash = Self::hash_string(&config_json);
        
        let created_at = chrono::Utc::now().to_rfc3339();
        let state_hash = Self::hash_string(&created_at);
        
        let fingerprint = Self::hash_string(&format!("{}:{}", config_hash, state_hash));
        
        let hashed_fields = Self::extract_fields(config)?;
        
        Ok(Self {
            config_hash,
            state_hash,
            fingerprint,
            created_at,
            hashed_fields,
            algorithm: "SHA-256".to_string(),
        })
    }
    
    /// Verify footprint matches current configuration
    pub fn verify<T: Serialize>(&self, config: &T) -> Result<bool> {
        let config_json = serde_json::to_string(config)?;
        let current_hash = Self::hash_string(&config_json);
        Ok(current_hash == self.config_hash)
    }
    
    fn hash_string(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    fn extract_fields<T: Serialize>(config: &T) -> Result<Vec<String>> {
        let value = serde_json::to_value(config)?;
        if let Some(obj) = value.as_object() {
            Ok(obj.keys().cloned().collect())
        } else {
            Ok(vec![])
        }
    }
}
```

### Enhanced InterfaceBinding

```rust
/// Interface binding with hash footprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceBinding {
    pub proxmox_veth: String,
    pub ovs_interface: String,
    pub vmid: u32,
    pub container_id: String,
    pub bridge: String,
    pub created_at: String,
    pub bind_mount: String,
    
    // NEW: Hash footprint for integrity
    pub footprint: HashFootprint,
    
    // NEW: Link to blockchain ledger
    pub ledger_block_hash: Option<String>,
}

impl InterfaceBinding {
    pub fn new(
        proxmox_veth: String,
        ovs_interface: String,
        vmid: u32,
        container_id: String,
        bridge: String,
    ) -> Result<Self> {
        let created_at = chrono::Utc::now().to_rfc3339();
        let bind_mount = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);
        
        // Create temporary binding without footprint
        let temp_binding = InterfaceBinding {
            proxmox_veth: proxmox_veth.clone(),
            ovs_interface: ovs_interface.clone(),
            vmid,
            container_id: container_id.clone(),
            bridge: bridge.clone(),
            created_at: created_at.clone(),
            bind_mount: bind_mount.clone(),
            footprint: HashFootprint::default(),
            ledger_block_hash: None,
        };
        
        // Generate footprint from the binding
        let footprint = HashFootprint::from_config(&temp_binding)?;
        
        Ok(InterfaceBinding {
            proxmox_veth,
            ovs_interface,
            vmid,
            container_id,
            bridge,
            created_at,
            bind_mount,
            footprint,
            ledger_block_hash: None,
        })
    }
    
    /// Verify binding hasn't been tampered with
    pub fn verify_integrity(&self) -> Result<bool> {
        self.footprint.verify(self)
    }
    
    /// Get the unique fingerprint
    pub fn fingerprint(&self) -> &str {
        &self.footprint.fingerprint
    }
}
```

## 🔄 Integration with Blockchain Ledger

```rust
/// Enhanced binding with ledger integration
pub fn bind_veth_interface_enhanced(
    proxmox_veth: &str,
    ovs_interface: &str,
    vmid: u32,
    container_id: &str,
    bridge: &str,
    ledger_path: &str,
) -> Result<InterfaceBinding> {
    // Create binding with footprint
    let mut binding = InterfaceBinding::new(
        proxmox_veth.to_string(),
        ovs_interface.to_string(),
        vmid,
        container_id.to_string(),
        bridge.to_string(),
    )?;
    
    // Add to blockchain ledger
    let mut ledger = BlockchainLedger::new(ledger_path.into())?;
    let ledger_data = json!({
        "interface": ovs_interface,
        "vmid": vmid,
        "fingerprint": binding.footprint.fingerprint,
        "config_hash": binding.footprint.config_hash,
    });
    
    let block_hash = ledger.add_data("interface", "created", ledger_data)?;
    binding.ledger_block_hash = Some(block_hash);
    
    // Create the bind mount
    bind_veth_interface(proxmox_veth, ovs_interface)?;
    
    // Store with footprint
    store_binding_info(&binding)?;
    
    info!(
        "Enhanced binding created with fingerprint: {} (block: {})",
        binding.fingerprint(),
        binding.ledger_block_hash.as_ref().unwrap()
    );
    
    Ok(binding)
}
```

## 📊 Hash Footprint Database

### Storage Format

```json
{
  "bindings": {
    "veth-101-eth0": {
      "proxmox_veth": "veth-101-eth0",
      "ovs_interface": "vi101",
      "vmid": 101,
      "container_id": "abc123",
      "bridge": "ovsbr0",
      "created_at": "2025-10-13T10:30:00Z",
      "bind_mount": "/var/lib/ovs-port-agent/fuse/vi101",
      "footprint": {
        "config_hash": "a1b2c3d4...",
        "state_hash": "e5f6g7h8...",
        "fingerprint": "9i0j1k2l...",
        "created_at": "2025-10-13T10:30:00Z",
        "hashed_fields": ["proxmox_veth", "ovs_interface", "vmid", ...],
        "algorithm": "SHA-256"
      },
      "ledger_block_hash": "block_hash_xyz"
    }
  },
  "fingerprint_index": {
    "9i0j1k2l...": "veth-101-eth0"
  }
}
```

### Content-Addressable Lookup

```rust
/// Find binding by fingerprint (content-addressable)
pub fn find_by_fingerprint(fingerprint: &str) -> Result<Option<InterfaceBinding>> {
    let bindings = get_interface_bindings()?;
    
    for (_, binding) in bindings {
        if binding.footprint.fingerprint == fingerprint {
            return Ok(Some(binding));
        }
    }
    
    Ok(None)
}

/// Detect duplicate configurations
pub fn find_duplicates() -> Result<Vec<Vec<InterfaceBinding>>> {
    let bindings = get_interface_bindings()?;
    let mut by_config_hash: HashMap<String, Vec<InterfaceBinding>> = HashMap::new();
    
    for (_, binding) in bindings {
        by_config_hash
            .entry(binding.footprint.config_hash.clone())
            .or_default()
            .push(binding);
    }
    
    Ok(by_config_hash
        .into_values()
        .filter(|v| v.len() > 1)
        .collect())
}
```

## 🔍 Verification & Drift Detection

```rust
/// Verify all bindings integrity
pub fn verify_all_bindings() -> Result<Vec<String>> {
    let bindings = get_interface_bindings()?;
    let mut tampered = Vec::new();
    
    for (name, binding) in bindings {
        if !binding.verify_integrity()? {
            tampered.push(name);
        }
    }
    
    Ok(tampered)
}

/// Detect configuration drift
pub fn detect_drift(interface: &str) -> Result<DriftReport> {
    let bindings = get_interface_bindings()?;
    
    if let Some(binding) = bindings.get(interface) {
        // Get current state
        let current_state = get_current_interface_state(interface)?;
        
        // Compare with original footprint
        let original_hash = &binding.footprint.config_hash;
        let current_hash = HashFootprint::hash_string(&current_state);
        
        Ok(DriftReport {
            interface: interface.to_string(),
            original_hash: original_hash.clone(),
            current_hash,
            has_drifted: original_hash != &current_hash,
            created_at: binding.created_at.clone(),
            checked_at: chrono::Utc::now().to_rfc3339(),
        })
    } else {
        Err(anyhow::anyhow!("Interface not found: {}", interface))
    }
}

#[derive(Debug, Serialize)]
pub struct DriftReport {
    pub interface: String,
    pub original_hash: String,
    pub current_hash: String,
    pub has_drifted: bool,
    pub created_at: String,
    pub checked_at: String,
}
```

## 🎯 Use Cases

### 1. **Tamper Detection**

```bash
# Check if interface was modified
ovs-port-agent verify-integrity vi101

# Output:
# ✅ Interface vi101: Integrity OK
# 🔒 Fingerprint: 9i0j1k2l...
# 📅 Created: 2025-10-13T10:30:00Z

# Or if tampered:
# ❌ Interface vi101: INTEGRITY VIOLATION!
# 🚨 Current hash: a1b2c3... != Original: d4e5f6...
# ⚠️  Interface may have been modified outside of ovs-port-agent
```

### 2. **Configuration Deduplication**

```bash
# Before creating new binding, check for duplicate
ovs-port-agent create --vmid 102 --bridge ovsbr0

# Output:
# ℹ️  Found existing binding with identical config
# 🔗 Fingerprint: 9i0j1k2l...
# ♻️  Reusing existing binding for efficiency
```

### 3. **Drift Detection**

```bash
# Periodic drift check
ovs-port-agent check-drift vi101

# Output:
# 📊 Drift Report for vi101:
# ⚠️  Configuration has drifted!
# Original (2025-10-13): a1b2c3d4...
# Current  (2025-10-13): e5f6g7h8...
# 
# Differences detected:
# - bridge: ovsbr0 → ovsbr1 (changed)
# - vmid: 101 (unchanged)
```

### 4. **Audit Trail Verification**

```bash
# Verify binding matches blockchain record
ovs-port-agent audit-verify vi101

# Output:
# 🔍 Verifying vi101 against blockchain ledger
# 📦 Block hash: block_xyz
# ✅ Fingerprint matches ledger: 9i0j1k2l...
# ✅ Blockchain integrity: VERIFIED
# ✅ Binding integrity: VERIFIED
```

## 🚀 Implementation Plan

### Phase 1: Core Hash Footprint (1-2 hours)

1. Add `HashFootprint` struct to `src/fuse.rs`
2. Update `InterfaceBinding` with footprint field
3. Modify `bind_veth_interface_enhanced()` to generate footprints
4. Update storage functions to preserve footprints

### Phase 2: Verification (1 hour)

1. Add `verify_integrity()` method
2. Add `verify_all_bindings()` function
3. Create D-Bus method for verification

### Phase 3: Drift Detection (1 hour)

1. Add `detect_drift()` function
2. Create `DriftReport` struct
3. Periodic drift checking service

### Phase 4: Deduplication (1 hour)

1. Add fingerprint index
2. Implement `find_by_fingerprint()`
3. Check for duplicates before creation

### Phase 5: CLI & D-Bus API (1 hour)

1. Add CLI commands: `verify-integrity`, `check-drift`, `find-duplicates`
2. Add D-Bus methods for footprint operations
3. Documentation

## 📈 Performance Impact

### Minimal Overhead

- **Hash generation**: ~100μs per binding (SHA-256 is fast)
- **Storage**: +200 bytes per binding (footprint data)
- **Verification**: ~50μs (hash comparison)

### Benefits

- **Deduplication savings**: Prevent duplicate bindings
- **Early detection**: Catch tampering immediately
- **Audit confidence**: Cryptographic proof

## 🔐 Security Considerations

### Strengths

✅ **Cryptographic integrity** - SHA-256 provides strong guarantees
✅ **Tamper detection** - Any modification changes hash
✅ **Audit trail** - Linked to blockchain ledger
✅ **Content-addressable** - Unique fingerprints

### Limitations

⚠️ **No authentication** - Footprint doesn't prove who created it
⚠️ **No encryption** - Data is hashed, not encrypted
⚠️ **Replay attacks** - Old config could be replayed

### Mitigations

1. Link to blockchain ledger (provides timestamp + chain of custody)
2. Include nonce/timestamp in hash (prevents replay)
3. Consider HMAC for authenticated hashing

## 📚 References

- **Content-Addressable Storage**: IPFS, Git use similar concepts
- **Hash-based verification**: Docker image digests, Nix store
- **Blockchain integration**: Existing ledger.rs provides chain of custody

---

**Next Steps**: Implement Phase 1 (Core Hash Footprint)
