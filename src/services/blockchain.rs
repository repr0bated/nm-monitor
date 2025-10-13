//! Blockchain ledger service for audit logging and data integrity

use crate::ledger::BlockchainLedger;
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

    /// Get blockchain statistics
    pub fn get_stats(&self) -> Result<crate::ledger::BlockchainStats> {
        debug!("Getting blockchain statistics from {:?}", self.ledger_path);
        let ledger = BlockchainLedger::new(self.ledger_path.clone())
            .with_context(|| format!("Failed to open ledger at {:?}", self.ledger_path))?;

        ledger
            .get_stats()
            .context("Failed to retrieve blockchain statistics")
    }

    /// Get blocks by category
    pub fn get_blocks_by_category(&self, category: &str) -> Result<Vec<crate::ledger::Block>> {
        debug!(
            "Getting blocks for category '{}' from {:?}",
            category, self.ledger_path
        );
        let ledger = BlockchainLedger::new(self.ledger_path.clone())
            .with_context(|| format!("Failed to open ledger at {:?}", self.ledger_path))?;

        ledger
            .get_blocks_by_category(category)
            .with_context(|| format!("Failed to get blocks for category '{}'", category))
    }

    /// Get blocks by height range
    pub fn get_blocks_by_height(
        &self,
        start: u64,
        end: u64,
    ) -> Result<Vec<crate::ledger::Block>> {
        debug!(
            "Getting blocks from height {} to {} from {:?}",
            start, end, self.ledger_path
        );
        
        if start > end {
            anyhow::bail!("Invalid height range: start ({}) > end ({})", start, end);
        }

        let ledger = BlockchainLedger::new(self.ledger_path.clone())
            .with_context(|| format!("Failed to open ledger at {:?}", self.ledger_path))?;

        ledger
            .get_blocks_by_height(start, end)
            .with_context(|| format!("Failed to get blocks in range {}-{}", start, end))
    }

    /// Verify blockchain integrity
    pub fn verify_chain(&self) -> Result<bool> {
        info!("Verifying blockchain integrity at {:?}", self.ledger_path);
        let ledger = BlockchainLedger::new(self.ledger_path.clone())
            .with_context(|| format!("Failed to open ledger at {:?}", self.ledger_path))?;

        ledger
            .verify_chain()
            .context("Failed to verify blockchain integrity")
    }

    /// Add data to blockchain
    pub fn add_data(&self, category: &str, action: &str, data: JsonValue) -> Result<String> {
        debug!(
            "Adding data to blockchain: category='{}', action='{}'",
            category, action
        );
        let mut ledger = BlockchainLedger::new(self.ledger_path.clone())
            .with_context(|| format!("Failed to open ledger at {:?}", self.ledger_path))?;

        ledger
            .add_data(category, action, data)
            .with_context(|| {
                format!(
                    "Failed to add data to blockchain: category='{}', action='{}'",
                    category, action
                )
            })
    }

    /// Get specific block by hash
    pub fn get_block_by_hash(&self, hash: &str) -> Result<Option<crate::ledger::Block>> {
        debug!("Getting block with hash '{}' from {:?}", hash, self.ledger_path);
        let ledger = BlockchainLedger::new(self.ledger_path.clone())
            .with_context(|| format!("Failed to open ledger at {:?}", self.ledger_path))?;

        ledger
            .get_block(hash)
            .with_context(|| format!("Failed to get block with hash '{}'", hash))
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
        assert_eq!(stats.total_blocks, 1);
    }

    #[test]
    fn test_verify_empty_chain() {
        let temp_dir = TempDir::new().unwrap();
        let ledger_path = temp_dir.path().join("test_ledger.jsonl");
        let service = BlockchainService::new(&ledger_path);

        let result = service.verify_chain();
        assert!(result.is_ok());
        assert!(result.unwrap()); // Empty chain is valid
    }

    #[test]
    fn test_invalid_height_range() {
        let temp_dir = TempDir::new().unwrap();
        let ledger_path = temp_dir.path().join("test_ledger.jsonl");
        let service = BlockchainService::new(&ledger_path);

        let result = service.get_blocks_by_height(10, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid height range"));
    }
}
