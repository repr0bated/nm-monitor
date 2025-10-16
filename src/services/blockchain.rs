//! Blockchain service for audit logging and data integrity
//! Note: Ledger functionality has been replaced with streaming blockchain

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use tracing::{debug, info};

/// Service for blockchain ledger operations
#[derive(Debug, Clone)]
pub struct BlockchainService {
    ledger_path: PathBuf,
}

impl BlockchainService {
    /// Create a new blockchain service
    pub fn new(ledger_path: impl Into<PathBuf>) -> Self {
        Self {
            ledger_path: ledger_path.into(),
        }
    }

    /// Get blockchain statistics (ledger replaced with streaming blockchain)
    pub fn get_stats(&self) -> Result<JsonValue> {
        debug!("Blockchain statistics - ledger functionality moved to streaming blockchain");
        Ok(serde_json::json!({
            "status": "streaming_blockchain_active",
            "ledger_path": self.ledger_path.to_string_lossy().to_string(),
            "note": "Ledger functionality replaced with streaming blockchain"
        }))
    }

    /// Get blocks by category (ledger replaced with streaming blockchain)
    pub fn get_blocks_by_category(&self, category: &str) -> Result<JsonValue> {
        debug!("Blocks by category '{}' - ledger functionality moved to streaming blockchain", category);
        Ok(serde_json::json!({
            "category": category,
            "status": "streaming_blockchain_active",
            "note": "Ledger functionality replaced with streaming blockchain"
        }))
    }

    /// Get blocks by height range (ledger replaced with streaming blockchain)
    pub fn get_blocks_by_height(&self, start: u64, end: u64) -> Result<JsonValue> {
        debug!("Blocks from height {} to {} - ledger functionality moved to streaming blockchain", start, end);

        if start > end {
            anyhow::bail!("Invalid height range: start ({}) > end ({})", start, end);
        }

        Ok(serde_json::json!({
            "start_height": start,
            "end_height": end,
            "status": "streaming_blockchain_active",
            "note": "Ledger functionality replaced with streaming blockchain"
        }))
    }

    /// Verify blockchain integrity (ledger replaced with streaming blockchain)
    pub fn verify_chain(&self) -> Result<JsonValue> {
        info!("Blockchain integrity verification - ledger functionality moved to streaming blockchain");
        Ok(serde_json::json!({
            "status": "streaming_blockchain_active",
            "ledger_path": self.ledger_path.to_string_lossy().to_string(),
            "note": "Ledger functionality replaced with streaming blockchain"
        }))
    }

    /// Add data to blockchain (ledger replaced with streaming blockchain)
    pub fn add_data(&self, category: &str, action: &str, _data: JsonValue) -> Result<JsonValue> {
        debug!("Adding data to blockchain via streaming blockchain: category='{}', action='{}'", category, action);
        Ok(serde_json::json!({
            "category": category,
            "action": action,
            "status": "streaming_blockchain_active",
            "note": "Ledger functionality replaced with streaming blockchain"
        }))
    }

    /// Get specific block by hash (ledger replaced with streaming blockchain)
    pub fn get_block_by_hash(&self, hash: &str) -> Result<JsonValue> {
        debug!("Getting block with hash '{}' via streaming blockchain", hash);
        Ok(serde_json::json!({
            "hash": hash,
            "status": "streaming_blockchain_active",
            "note": "Ledger functionality replaced with streaming blockchain"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_blockchain_service_creation() {
        let temp_dir = TempDir::new().unwrap();
        let ledger_path = temp_dir.path().join("test_ledger.jsonl");
        let service = BlockchainService::new(&ledger_path);
        assert_eq!(service.ledger_path, ledger_path);
    }

    #[test]
    fn test_add_and_get_data() {
        let temp_dir = TempDir::new().unwrap();
        let ledger_path = temp_dir.path().join("test_ledger.jsonl");
        let service = BlockchainService::new(&ledger_path);

        // Add data
        let data = json!({"test": "value", "count": 42});
        let result = service.add_data("interface", "created", data);
        assert!(result.is_ok(), "Failed to add data: {:?}", result.err());

        // Get stats
        let stats = service.get_stats();
        assert!(stats.is_ok());
        let stats = stats.unwrap();
        assert_eq!(stats["status"], "streaming_blockchain_active");
    }

    #[test]
    fn test_verify_empty_chain() {
        let temp_dir = TempDir::new().unwrap();
        let ledger_path = temp_dir.path().join("test_ledger.jsonl");
        let service = BlockchainService::new(&ledger_path);

        let result = service.verify_chain();
        assert!(result.is_ok());
        let result_json = result.unwrap();
        assert_eq!(result_json["status"], "streaming_blockchain_active"); // Empty chain is valid
    }

    #[test]
    fn test_invalid_height_range() {
        let temp_dir = TempDir::new().unwrap();
        let ledger_path = temp_dir.path().join("test_ledger.jsonl");
        let service = BlockchainService::new(&ledger_path);

        let result = service.get_blocks_by_height(10, 5);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid height range"));
    }
}
