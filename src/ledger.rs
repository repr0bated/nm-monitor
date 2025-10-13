use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

/// Core blockchain record with comprehensive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Unix timestamp of block creation
    pub timestamp: u64,
    /// Block height in the chain
    pub height: u64,
    /// Previous block hash (empty for genesis block)
    pub prev_hash: String,
    /// Current block hash
    pub hash: String,
    /// Block producer/source identifier
    pub producer: String,
    /// Category of data (settings, users, storage, connections, etc.)
    pub category: String,
    /// Action type within category
    pub action: String,
    /// User/system that triggered the change
    pub user: String,
    /// Host system identifier
    pub hostname: String,
    /// Process ID that created the block
    pub pid: u32,
    /// Actual data payload
    pub data: serde_json::Value,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Digital signature (future enhancement)
    pub signature: Option<String>,
}

/// Blockchain ledger for comprehensive system accountability
pub struct BlockchainLedger {
    /// Path to the ledger file
    path: PathBuf,
    /// Current chain height
    height: u64,
    /// Hash of the last block
    last_hash: String,
    /// Genesis hash for verification
    #[allow(dead_code)]
    genesis_hash: String,
    /// Registered data source plugins
    plugins: HashMap<String, Box<dyn LedgerPlugin>>,
}

/// Plugin trait for different data sources
pub trait LedgerPlugin: Send + Sync {
    /// Get plugin name/identifier
    #[allow(dead_code)]
    fn name(&self) -> &str;
    /// Get data categories this plugin handles
    fn categories(&self) -> Vec<String>;
    /// Process data and return blocks to be added to chain
    fn process_data(&self, data: serde_json::Value) -> Result<Vec<Block>>;
    /// Validate data before adding to chain
    fn validate_data(&self, data: &serde_json::Value) -> Result<bool>;
}

/// Network operations plugin
pub struct NetworkPlugin;

impl LedgerPlugin for NetworkPlugin {
    fn name(&self) -> &str { "network" }

    fn categories(&self) -> Vec<String> {
        vec![
            "bridge".to_string(),
            "interface".to_string(),
            "connection".to_string(),
            "port".to_string(),
            "connectivity".to_string(),
        ]
    }

    fn process_data(&self, data: serde_json::Value) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        if let Some(category) = data.get("category").and_then(|c| c.as_str()) {
            if self.categories().contains(&category.to_string()) {
                if let Ok(block) = self.create_block(category, data.clone()) {
                    blocks.push(block);
                }
            }
        }

        Ok(blocks)
    }

    fn validate_data(&self, data: &serde_json::Value) -> Result<bool> {
        // Basic validation - ensure required fields exist
        let required_fields = ["action", "details"];
        for field in &required_fields {
            if !data.get(*field).is_some() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl NetworkPlugin {
    fn create_block(&self, category: &str, data: serde_json::Value) -> Result<Block> {
        let hostname = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(Block {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 0, // Will be set by ledger
            prev_hash: String::new(), // Will be set by ledger
            hash: String::new(), // Will be calculated by ledger
            producer: "network-plugin".to_string(),
            category: category.to_string(),
            action: data["action"].as_str().unwrap_or("unknown").to_string(),
            user: data["user"].as_str().unwrap_or("system").to_string(),
            hostname,
            pid: std::process::id(),
            data: data["details"].clone(),
            metadata: extract_metadata(&data),
            signature: None,
        })
    }
}

/// Settings/Configuration plugin
pub struct SettingsPlugin;

impl LedgerPlugin for SettingsPlugin {
    fn name(&self) -> &str { "settings" }

    fn categories(&self) -> Vec<String> {
        vec![
            "config".to_string(),
            "policy".to_string(),
            "template".to_string(),
            "parameter".to_string(),
        ]
    }

    fn process_data(&self, data: serde_json::Value) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        if let Some(category) = data.get("category").and_then(|c| c.as_str()) {
            if self.categories().contains(&category.to_string()) {
                if let Ok(block) = self.create_block(category, data.clone()) {
                    blocks.push(block);
                }
            }
        }

        Ok(blocks)
    }

    fn validate_data(&self, data: &serde_json::Value) -> Result<bool> {
        // Validate settings have proper structure
        if let Some(settings) = data.get("settings") {
            if settings.is_object() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl SettingsPlugin {
    fn create_block(&self, category: &str, data: serde_json::Value) -> Result<Block> {
        let hostname = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(Block {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 0,
            prev_hash: String::new(),
            hash: String::new(),
            producer: "settings-plugin".to_string(),
            category: category.to_string(),
            action: data["action"].as_str().unwrap_or("modified").to_string(),
            user: data["user"].as_str().unwrap_or("admin").to_string(),
            hostname,
            pid: std::process::id(),
            data: data["settings"].clone(),
            metadata: extract_metadata(&data),
            signature: None,
        })
    }
}

/// User management plugin
pub struct UserPlugin;

impl LedgerPlugin for UserPlugin {
    fn name(&self) -> &str { "users" }

    fn categories(&self) -> Vec<String> {
        vec![
            "authentication".to_string(),
            "authorization".to_string(),
            "session".to_string(),
            "access".to_string(),
        ]
    }

    fn process_data(&self, data: serde_json::Value) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        if let Some(category) = data.get("category").and_then(|c| c.as_str()) {
            if self.categories().contains(&category.to_string()) {
                if let Ok(block) = self.create_block(category, data.clone()) {
                    blocks.push(block);
                }
            }
        }

        Ok(blocks)
    }

    fn validate_data(&self, data: &serde_json::Value) -> Result<bool> {
        // Validate user data has required fields
        let required_fields = ["username", "action"];
        for field in &required_fields {
            if !data.get(*field).is_some() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl UserPlugin {
    fn create_block(&self, category: &str, data: serde_json::Value) -> Result<Block> {
        let hostname = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(Block {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 0,
            prev_hash: String::new(),
            hash: String::new(),
            producer: "user-plugin".to_string(),
            category: category.to_string(),
            action: data["action"].as_str().unwrap_or("unknown").to_string(),
            user: data["username"].as_str().unwrap_or("unknown").to_string(),
            hostname,
            pid: std::process::id(),
            data: data["details"].clone(),
            metadata: extract_metadata(&data),
            signature: None,
        })
    }
}

/// Storage operations plugin
pub struct StoragePlugin;

impl LedgerPlugin for StoragePlugin {
    fn name(&self) -> &str { "storage" }

    fn categories(&self) -> Vec<String> {
        vec![
            "filesystem".to_string(),
            "mount".to_string(),
            "backup".to_string(),
            "snapshot".to_string(),
        ]
    }

    fn process_data(&self, data: serde_json::Value) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        if let Some(category) = data.get("category").and_then(|c| c.as_str()) {
            if self.categories().contains(&category.to_string()) {
                if let Ok(block) = self.create_block(category, data.clone()) {
                    blocks.push(block);
                }
            }
        }

        Ok(blocks)
    }

    fn validate_data(&self, data: &serde_json::Value) -> Result<bool> {
        // Validate storage operations have path information
        if data.get("path").is_some() || data.get("device").is_some() {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl StoragePlugin {
    fn create_block(&self, category: &str, data: serde_json::Value) -> Result<Block> {
        let hostname = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(Block {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            height: 0,
            prev_hash: String::new(),
            hash: String::new(),
            producer: "storage-plugin".to_string(),
            category: category.to_string(),
            action: data["action"].as_str().unwrap_or("unknown").to_string(),
            user: data["user"].as_str().unwrap_or("system").to_string(),
            hostname,
            pid: std::process::id(),
            data: data["details"].clone(),
            metadata: extract_metadata(&data),
            signature: None,
        })
    }
}

impl BlockchainLedger {
    /// Create new blockchain ledger with plugin system
    pub fn new(path: PathBuf) -> Result<Self> {
        let mut height = 0u64;
        let mut last_hash = String::new();
        let mut genesis_hash = String::new();

        // Load existing chain if present
        if let Ok(data) = fs::read_to_string(&path) {
            for line in data.lines() {
                if let Ok(block) = serde_json::from_str::<Block>(line) {
                    height = block.height;
                    last_hash = block.hash.clone();
                    if genesis_hash.is_empty() {
                        genesis_hash = block.hash.clone();
                    }
                }
            }
        }

        let mut plugins: HashMap<String, Box<dyn LedgerPlugin>> = HashMap::new();
        plugins.insert("network".to_string(), Box::new(NetworkPlugin));
        plugins.insert("settings".to_string(), Box::new(SettingsPlugin));
        plugins.insert("users".to_string(), Box::new(UserPlugin));
        plugins.insert("storage".to_string(), Box::new(StoragePlugin));

        Ok(Self {
            path,
            height,
            last_hash,
            genesis_hash,
            plugins,
        })
    }

    /// Add block to the blockchain with validation
    pub fn add_block(&mut self, block: Block) -> Result<String> {
        // Validate block structure
        self.validate_block(&block)?;

        // Calculate block hash
        let hash = self.calculate_hash(&block)?;

        // Create complete block
        let mut complete_block = block;
        complete_block.height = self.height + 1;
        complete_block.prev_hash = self.last_hash.clone();
        complete_block.hash = hash;

        // Verify hash is correct
        let verify_hash = self.calculate_hash(&complete_block)?;
        if verify_hash != complete_block.hash {
            return Err(anyhow::anyhow!("Hash verification failed"));
        }

        // Write to ledger file
        let line = serde_json::to_string(&complete_block)? + "\n";
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open ledger {}", self.path.display()))?;

        f.write_all(line.as_bytes())
            .with_context(|| "append to ledger")?;

        // Update chain state
        self.height = complete_block.height;
        self.last_hash = complete_block.hash.clone();

        info!("Added block {} to blockchain (category: {}, action: {})",
              complete_block.height, complete_block.category, complete_block.action);

        Ok(complete_block.hash)
    }

    /// Add data through plugin system
    pub fn add_data(&mut self, category: &str, action: &str, data: serde_json::Value) -> Result<String> {
        let input = serde_json::json!({
            "category": category,
            "action": action,
            "details": data,
            "user": "system",
            "hostname": hostname::get().unwrap_or_default().to_string_lossy(),
            "pid": std::process::id(),
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        });

        // Find appropriate plugin
        for (_plugin_name, plugin) in &self.plugins {
            if plugin.categories().contains(&category.to_string()) {
                if plugin.validate_data(&input)? {
                    let blocks = plugin.process_data(input.clone())?;
                    for block in blocks {
                        return self.add_block(block);
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No suitable plugin found for category: {}", category))
    }

    /// Register custom plugin
    #[allow(dead_code)]
    pub fn register_plugin(&mut self, plugin: Box<dyn LedgerPlugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    /// Get block by hash
    pub fn get_block(&self, hash: &str) -> Result<Option<Block>> {
        if let Ok(data) = fs::read_to_string(&self.path) {
            for line in data.lines() {
                if let Ok(block) = serde_json::from_str::<Block>(line) {
                    if block.hash == hash {
                        return Ok(Some(block));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Get blocks by category
    pub fn get_blocks_by_category(&self, category: &str) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        if let Ok(data) = fs::read_to_string(&self.path) {
            for line in data.lines() {
                if let Ok(block) = serde_json::from_str::<Block>(line) {
                    if block.category == category {
                        blocks.push(block);
                    }
                }
            }
        }

        Ok(blocks)
    }

    /// Get blocks by height range
    pub fn get_blocks_by_height(&self, start: u64, end: u64) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        if let Ok(data) = fs::read_to_string(&self.path) {
            for line in data.lines() {
                if let Ok(block) = serde_json::from_str::<Block>(line) {
                    if block.height >= start && block.height <= end {
                        blocks.push(block);
                    }
                }
            }
        }

        Ok(blocks)
    }

    /// Verify blockchain integrity
    pub fn verify_chain(&self) -> Result<bool> {
        let mut prev_hash = String::new();
        let mut height = 0u64;

        if let Ok(data) = fs::read_to_string(&self.path) {
            for line in data.lines() {
                if let Ok(block) = serde_json::from_str::<Block>(line) {
                    // Verify height sequence
                    if block.height != height + 1 {
                        warn!("Height sequence broken at block {}", block.height);
                        return Ok(false);
                    }

                    // Verify hash chain
                    if block.prev_hash != prev_hash {
                        warn!("Hash chain broken at block {}", block.height);
                        return Ok(false);
                    }

                    // Verify block hash
                    let calculated_hash = self.calculate_hash(&block)?;
                    if calculated_hash != block.hash {
                        warn!("Block hash invalid at block {}", block.height);
                        return Ok(false);
                    }

                    prev_hash = block.hash.clone();
                    height = block.height;
                }
            }
        }

        // Verify final state matches expected
        if height != self.height || prev_hash != self.last_hash {
            warn!("Final chain state doesn't match expected");
            return Ok(false);
        }

        Ok(true)
    }

    /// Get blockchain statistics
    pub fn get_stats(&self) -> Result<BlockchainStats> {
        let mut stats = BlockchainStats::default();

        if let Ok(data) = fs::read_to_string(&self.path) {
            for line in data.lines() {
                if let Ok(block) = serde_json::from_str::<Block>(line) {
                    stats.total_blocks += 1;
                    stats.last_timestamp = block.timestamp;

                    // Count by category
                    *stats.categories.entry(block.category.clone()).or_insert(0) += 1;

                    // Count by producer
                    *stats.producers.entry(block.producer.clone()).or_insert(0) += 1;

                    // Track unique users
                    if block.user != "system" {
                        stats.unique_users.insert(block.user.clone());
                    }
                }
            }
        }

        Ok(stats)
    }

    fn validate_block(&self, block: &Block) -> Result<()> {
        // Basic validation
        if block.category.is_empty() {
            return Err(anyhow::anyhow!("Block category cannot be empty"));
        }

        if block.action.is_empty() {
            return Err(anyhow::anyhow!("Block action cannot be empty"));
        }

        if block.producer.is_empty() {
            return Err(anyhow::anyhow!("Block producer cannot be empty"));
        }

        Ok(())
    }

    fn calculate_hash(&self, block: &Block) -> Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(block.height.to_le_bytes());
        hasher.update(block.prev_hash.as_bytes());
        hasher.update(block.timestamp.to_le_bytes());
        hasher.update(block.producer.as_bytes());
        hasher.update(block.category.as_bytes());
        hasher.update(block.action.as_bytes());
        hasher.update(block.user.as_bytes());
        hasher.update(block.hostname.as_bytes());
        hasher.update(block.pid.to_le_bytes());
        hasher.update(block.data.to_string().as_bytes());

        // Include metadata in hash
        let mut metadata_str = String::new();
        let mut keys: Vec<&String> = block.metadata.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(value) = block.metadata.get(key) {
                metadata_str.push_str(&format!("{}:{}", key, value));
            }
        }
        hasher.update(metadata_str.as_bytes());

        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BlockchainStats {
    pub total_blocks: u64,
    pub last_timestamp: u64,
    pub categories: HashMap<String, u64>,
    pub producers: HashMap<String, u64>,
    pub unique_users: std::collections::HashSet<String>,
}

/// Utility functions
fn extract_metadata(data: &serde_json::Value) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    if let Some(meta) = data.get("metadata") {
        if let Some(meta_obj) = meta.as_object() {
            for (key, value) in meta_obj {
                if let Some(str_value) = value.as_str() {
                    metadata.insert(key.clone(), str_value.to_string());
                }
            }
        }
    }

    metadata
}

/// Legacy compatibility - simple append function
pub struct Ledger {
    inner: BlockchainLedger,
}

impl Ledger {
    pub fn open(path: PathBuf) -> Result<Self> {
        let inner = BlockchainLedger::new(path)?;
        Ok(Self { inner })
    }

    pub fn append(&mut self, action: &str, details: serde_json::Value) -> Result<()> {
        self.inner.add_data("interface", action, details)?;
        Ok(())
    }
}
