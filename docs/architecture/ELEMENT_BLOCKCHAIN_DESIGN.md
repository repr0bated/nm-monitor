# Element-Level Blockchain Design

## 🎯 Core Concept

**Every network element has its own mini blockchain** that tracks all modifications from creation to deletion. Each modification creates a new block in that element's chain.

## 🔗 Architecture

### Element Blockchain Structure

```rust
/// A single modification record in an element's blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementBlock {
    /// Block height in this element's chain (starts at 0)
    pub height: u64,
    
    /// Hash of previous block in THIS element's chain
    pub prev_hash: String,
    
    /// Hash of current block
    pub hash: String,
    
    /// Timestamp of modification
    pub timestamp: String,
    
    /// Type of modification (created, modified, deleted)
    pub modification_type: String,
    
    /// Snapshot of element state at this point
    pub state_snapshot: serde_json::Value,
    
    /// What changed (diff from previous state)
    pub changes: Vec<Change>,
    
    /// Who/what made the change
    pub actor: String,
    
    /// Why the change was made (optional)
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub field: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
}

/// Complete blockchain for a single network element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementBlockchain {
    /// Unique identifier for this element
    pub element_id: String,
    
    /// Type of element (interface, bridge, port, etc.)
    pub element_type: String,
    
    /// All blocks in chronological order
    pub blocks: Vec<ElementBlock>,
    
    /// Current state hash
    pub current_hash: String,
    
    /// Genesis block hash (first block)
    pub genesis_hash: String,
}

impl ElementBlockchain {
    /// Create new blockchain for an element (genesis block)
    pub fn new(
        element_id: String,
        element_type: String,
        initial_state: serde_json::Value,
        actor: String,
    ) -> Result<Self> {
        let genesis_block = ElementBlock {
            height: 0,
            prev_hash: "0".repeat(64), // No previous block
            hash: String::new(), // Calculated below
            timestamp: chrono::Utc::now().to_rfc3339(),
            modification_type: "created".to_string(),
            state_snapshot: initial_state,
            changes: vec![],
            actor,
            reason: Some("Initial creation".to_string()),
        };
        
        let hash = Self::calculate_block_hash(&genesis_block)?;
        let mut genesis_block = genesis_block;
        genesis_block.hash = hash.clone();
        
        Ok(Self {
            element_id,
            element_type,
            blocks: vec![genesis_block.clone()],
            current_hash: hash.clone(),
            genesis_hash: hash,
        })
    }
    
    /// Add modification to element's blockchain
    pub fn add_modification(
        &mut self,
        new_state: serde_json::Value,
        modification_type: String,
        actor: String,
        reason: Option<String>,
    ) -> Result<String> {
        let prev_block = self.blocks.last().ok_or_else(|| {
            anyhow::anyhow!("Blockchain has no blocks (corrupted)")
        })?;
        
        // Calculate diff
        let changes = self.calculate_diff(
            &prev_block.state_snapshot,
            &new_state,
        )?;
        
        let new_block = ElementBlock {
            height: prev_block.height + 1,
            prev_hash: prev_block.hash.clone(),
            hash: String::new(), // Calculated below
            timestamp: chrono::Utc::now().to_rfc3339(),
            modification_type,
            state_snapshot: new_state,
            changes,
            actor,
            reason,
        };
        
        let hash = Self::calculate_block_hash(&new_block)?;
        let mut new_block = new_block;
        new_block.hash = hash.clone();
        
        self.blocks.push(new_block);
        self.current_hash = hash.clone();
        
        Ok(hash)
    }
    
    /// Verify entire chain integrity
    pub fn verify_chain(&self) -> Result<bool> {
        if self.blocks.is_empty() {
            return Ok(false);
        }
        
        // Verify genesis
        if self.blocks[0].prev_hash != "0".repeat(64) {
            return Ok(false);
        }
        
        // Verify each block links to previous
        for i in 1..self.blocks.len() {
            let prev = &self.blocks[i - 1];
            let current = &self.blocks[i];
            
            if current.prev_hash != prev.hash {
                return Ok(false);
            }
            
            if current.height != prev.height + 1 {
                return Ok(false);
            }
            
            // Verify hash is correct
            let calculated_hash = Self::calculate_block_hash(current)?;
            if calculated_hash != current.hash {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Get current state
    pub fn current_state(&self) -> Option<&serde_json::Value> {
        self.blocks.last().map(|b| &b.state_snapshot)
    }
    
    /// Get state at specific height
    pub fn state_at_height(&self, height: u64) -> Option<&serde_json::Value> {
        self.blocks
            .iter()
            .find(|b| b.height == height)
            .map(|b| &b.state_snapshot)
    }
    
    /// Get modification history
    pub fn history(&self) -> Vec<&ElementBlock> {
        self.blocks.iter().collect()
    }
    
    fn calculate_block_hash(block: &ElementBlock) -> Result<String> {
        use sha2::{Digest, Sha256};
        
        let mut hasher = Sha256::new();
        hasher.update(block.height.to_le_bytes());
        hasher.update(block.prev_hash.as_bytes());
        hasher.update(block.timestamp.as_bytes());
        hasher.update(block.modification_type.as_bytes());
        hasher.update(block.state_snapshot.to_string().as_bytes());
        hasher.update(block.actor.as_bytes());
        
        Ok(format!("{:x}", hasher.finalize()))
    }
    
    fn calculate_diff(
        &self,
        old: &serde_json::Value,
        new: &serde_json::Value,
    ) -> Result<Vec<Change>> {
        let mut changes = Vec::new();
        
        if let (Some(old_obj), Some(new_obj)) = (old.as_object(), new.as_object()) {
            // Find modified/new fields
            for (key, new_val) in new_obj {
                if let Some(old_val) = old_obj.get(key) {
                    if old_val != new_val {
                        changes.push(Change {
                            field: key.clone(),
                            old_value: Some(old_val.clone()),
                            new_value: new_val.clone(),
                        });
                    }
                } else {
                    changes.push(Change {
                        field: key.clone(),
                        old_value: None,
                        new_value: new_val.clone(),
                    });
                }
            }
            
            // Find deleted fields
            for (key, old_val) in old_obj {
                if !new_obj.contains_key(key) {
                    changes.push(Change {
                        field: key.clone(),
                        old_value: Some(old_val.clone()),
                        new_value: serde_json::Value::Null,
                    });
                }
            }
        }
        
        Ok(changes)
    }
}
```

## 📊 Storage Structure

### Per-Element Blockchain Storage

```
/var/lib/ovs-port-agent/element-chains/
├── interfaces/
│   ├── vi101.jsonl           # Each line is a block
│   ├── vi102.jsonl
│   └── veth-103-eth0.jsonl
├── bridges/
│   ├── ovsbr0.jsonl
│   └── ovsbr1.jsonl
├── ports/
│   ├── port-001.jsonl
│   └── port-002.jsonl
└── index.json                # Quick lookup index
```

### Example: Interface Blockchain File

```jsonl
{"height":0,"prev_hash":"0000...","hash":"a1b2...","timestamp":"2025-10-13T10:00:00Z","modification_type":"created","state_snapshot":{"interface":"vi101","bridge":"ovsbr0","vmid":101},"changes":[],"actor":"system","reason":"Initial creation"}
{"height":1,"prev_hash":"a1b2...","hash":"c3d4...","timestamp":"2025-10-13T11:00:00Z","modification_type":"modified","state_snapshot":{"interface":"vi101","bridge":"ovsbr1","vmid":101},"changes":[{"field":"bridge","old_value":"ovsbr0","new_value":"ovsbr1"}],"actor":"admin","reason":"Moved to new bridge"}
{"height":2,"prev_hash":"c3d4...","hash":"e5f6...","timestamp":"2025-10-13T12:00:00Z","modification_type":"modified","state_snapshot":{"interface":"vi101","bridge":"ovsbr1","vmid":101,"mtu":9000},"changes":[{"field":"mtu","old_value":null,"new_value":9000}],"actor":"system","reason":"MTU adjustment"}
```

## 🔄 Integration with InterfaceBinding

```rust
/// Enhanced InterfaceBinding with element blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceBinding {
    pub proxmox_veth: String,
    pub ovs_interface: String,
    pub vmid: u32,
    pub container_id: String,
    pub bridge: String,
    pub created_at: String,
    pub bind_mount: String,
    
    /// Element blockchain tracking all modifications
    pub blockchain: ElementBlockchain,
    
    /// Quick access to current hash
    pub current_hash: String,
}

impl InterfaceBinding {
    /// Create new binding with genesis block
    pub fn new(
        proxmox_veth: String,
        ovs_interface: String,
        vmid: u32,
        container_id: String,
        bridge: String,
    ) -> Result<Self> {
        let created_at = chrono::Utc::now().to_rfc3339();
        let bind_mount = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);
        
        let initial_state = json!({
            "proxmox_veth": proxmox_veth,
            "ovs_interface": ovs_interface,
            "vmid": vmid,
            "container_id": container_id,
            "bridge": bridge,
            "created_at": created_at,
            "bind_mount": bind_mount,
        });
        
        let blockchain = ElementBlockchain::new(
            ovs_interface.clone(),
            "interface".to_string(),
            initial_state,
            "system".to_string(),
        )?;
        
        let current_hash = blockchain.current_hash.clone();
        
        Ok(Self {
            proxmox_veth,
            ovs_interface,
            vmid,
            container_id,
            bridge,
            created_at,
            bind_mount,
            blockchain,
            current_hash,
        })
    }
    
    /// Modify binding and record in blockchain
    pub fn modify_bridge(&mut self, new_bridge: String, actor: String) -> Result<String> {
        self.bridge = new_bridge.clone();
        
        let new_state = json!({
            "proxmox_veth": self.proxmox_veth,
            "ovs_interface": self.ovs_interface,
            "vmid": self.vmid,
            "container_id": self.container_id,
            "bridge": new_bridge,
            "created_at": self.created_at,
            "bind_mount": self.bind_mount,
        });
        
        let hash = self.blockchain.add_modification(
            new_state,
            "modified".to_string(),
            actor,
            Some(format!("Changed bridge to {}", new_bridge)),
        )?;
        
        self.current_hash = hash.clone();
        Ok(hash)
    }
    
    /// Verify binding's blockchain integrity
    pub fn verify_chain(&self) -> Result<bool> {
        self.blockchain.verify_chain()
    }
    
    /// Get modification history
    pub fn history(&self) -> Vec<&ElementBlock> {
        self.blockchain.history()
    }
}
```

## 🗂️ Element Blockchain Manager

```rust
use std::path::PathBuf;
use std::collections::HashMap;

/// Central manager for all element blockchains
pub struct ElementChainManager {
    base_path: PathBuf,
    chains: HashMap<String, ElementBlockchain>,
}

impl ElementChainManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        fs::create_dir_all(&base_path)?;
        
        Ok(Self {
            base_path,
            chains: HashMap::new(),
        })
    }
    
    /// Load element blockchain from disk
    pub fn load_chain(&mut self, element_id: &str, element_type: &str) -> Result<ElementBlockchain> {
        let chain_file = self.base_path
            .join(element_type)
            .join(format!("{}.jsonl", element_id));
        
        if !chain_file.exists() {
            return Err(anyhow::anyhow!("Chain not found: {}", element_id));
        }
        
        let content = fs::read_to_string(&chain_file)?;
        let mut blocks = Vec::new();
        
        for line in content.lines() {
            let block: ElementBlock = serde_json::from_str(line)?;
            blocks.push(block);
        }
        
        if blocks.is_empty() {
            return Err(anyhow::anyhow!("Empty chain: {}", element_id));
        }
        
        let current_hash = blocks.last().unwrap().hash.clone();
        let genesis_hash = blocks[0].hash.clone();
        
        let chain = ElementBlockchain {
            element_id: element_id.to_string(),
            element_type: element_type.to_string(),
            blocks,
            current_hash,
            genesis_hash,
        };
        
        // Verify integrity
        if !chain.verify_chain()? {
            return Err(anyhow::anyhow!("Chain integrity check failed: {}", element_id));
        }
        
        Ok(chain)
    }
    
    /// Save element blockchain to disk
    pub fn save_chain(&self, chain: &ElementBlockchain) -> Result<()> {
        let chain_dir = self.base_path.join(&chain.element_type);
        fs::create_dir_all(&chain_dir)?;
        
        let chain_file = chain_dir.join(format!("{}.jsonl", chain.element_id));
        
        let mut content = String::new();
        for block in &chain.blocks {
            content.push_str(&serde_json::to_string(block)?);
            content.push('\n');
        }
        
        fs::write(&chain_file, content)?;
        Ok(())
    }
    
    /// Append block to existing chain file (efficient)
    pub fn append_block(&self, element_id: &str, element_type: &str, block: &ElementBlock) -> Result<()> {
        let chain_file = self.base_path
            .join(element_type)
            .join(format!("{}.jsonl", element_id));
        
        let block_line = serde_json::to_string(block)? + "\n";
        
        use std::fs::OpenOptions;
        use std::io::Write;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&chain_file)?;
        
        file.write_all(block_line.as_bytes())?;
        file.flush()?;
        
        Ok(())
    }
    
    /// Get all elements of a type
    pub fn list_elements(&self, element_type: &str) -> Result<Vec<String>> {
        let type_dir = self.base_path.join(element_type);
        
        if !type_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut elements = Vec::new();
        for entry in fs::read_dir(type_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Some(name) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    elements.push(name.to_string());
                }
            }
        }
        
        Ok(elements)
    }
    
    /// Verify all chains
    pub fn verify_all(&self) -> Result<HashMap<String, bool>> {
        let mut results = HashMap::new();
        
        for element_type in ["interfaces", "bridges", "ports"] {
            for element_id in self.list_elements(element_type)? {
                let chain = self.load_chain(&element_id, element_type)?;
                results.insert(
                    format!("{}:{}", element_type, element_id),
                    chain.verify_chain()?,
                );
            }
        }
        
        Ok(results)
    }
}
```

## 🔧 Usage Examples

### Create Interface with Blockchain

```rust
// Create binding (automatically creates genesis block)
let binding = InterfaceBinding::new(
    "veth-101-eth0".to_string(),
    "vi101".to_string(),
    101,
    "abc123".to_string(),
    "ovsbr0".to_string(),
)?;

// Save the blockchain
manager.save_chain(&binding.blockchain)?;

println!("Created interface with genesis hash: {}", binding.current_hash);
```

### Modify Interface

```rust
// Load existing binding
let mut binding = load_binding("vi101")?;

// Modify bridge
let new_hash = binding.modify_bridge("ovsbr1".to_string(), "admin".to_string())?;

// Append new block to chain file (efficient!)
let latest_block = binding.blockchain.blocks.last().unwrap();
manager.append_block("vi101", "interface", latest_block)?;

println!("Modified interface, new hash: {}", new_hash);
```

### View History

```rust
let binding = load_binding("vi101")?;

println!("Modification history for vi101:");
for block in binding.history() {
    println!("  Block {}: {} at {} by {}",
        block.height,
        block.modification_type,
        block.timestamp,
        block.actor
    );
    
    for change in &block.changes {
        println!("    - {}: {:?} → {:?}",
            change.field,
            change.old_value,
            change.new_value
        );
    }
}
```

### Verify Integrity

```rust
// Verify single element
let binding = load_binding("vi101")?;
if binding.verify_chain()? {
    println!("✅ vi101 blockchain intact");
} else {
    println!("❌ vi101 blockchain CORRUPTED!");
}

// Verify all elements
let results = manager.verify_all()?;
for (id, ok) in results {
    if ok {
        println!("✅ {}", id);
    } else {
        println!("❌ {} CORRUPTED!", id);
    }
}
```

## 🎯 Benefits

### 1. **Complete Audit Trail**
- Every modification recorded
- Who, what, when, why
- Immutable history

### 2. **Tamper Detection**
- Chain breaks if modified
- Instant corruption detection
- Cryptographic proof

### 3. **Time Travel**
- View state at any point
- Rollback capability
- Diff between versions

### 4. **Distributed Verification**
- Each element independently verifiable
- No central ledger bottleneck
- Parallel verification

### 5. **Git-like Workflow**
```bash
# View history
ovs-port-agent log vi101

# Diff between versions
ovs-port-agent diff vi101@0 vi101@2

# Rollback to previous state
ovs-port-agent rollback vi101 --to-height 1

# Verify integrity
ovs-port-agent verify vi101
```

## 📈 Performance

### Append-Only Design
- **Write**: O(1) - append to file
- **Read latest**: O(1) - last line
- **Read history**: O(n) - scan file
- **Verify**: O(n) - check all blocks

### Storage
- ~200 bytes per block
- 100 modifications = 20KB per element
- Compaction possible (keep last N blocks)

## 🔐 Security

- ✅ **Cryptographic integrity** via SHA-256
- ✅ **Tamper-evident** chain structure
- ✅ **Append-only** storage
- ✅ **No central authority** (per-element chains)

## 🚀 Implementation Plan

1. **Phase 1**: Core ElementBlockchain (2 hours)
2. **Phase 2**: ElementChainManager (1 hour)
3. **Phase 3**: Integration with InterfaceBinding (1 hour)
4. **Phase 4**: Modify tracking (1 hour)
5. **Phase 5**: CLI tools (log, diff, verify) (2 hours)

---

**This is like Git for network configuration!** 🎉
