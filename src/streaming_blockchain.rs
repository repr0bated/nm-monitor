//! Streaming blockchain with vectorization and dual btrfs subvolumes

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use crate::plugin_footprint::PluginFootprint;

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockEvent {
    pub timestamp: u64,
    pub category: String,
    pub action: String,
    pub data: serde_json::Value,
    pub hash: String,
    pub vector: Vec<f32>, // Vectorized representation
}

pub struct StreamingBlockchain {
    base_path: PathBuf,
    timing_subvol: PathBuf,   // Subvolume 1: creation/timing data
    vector_subvol: PathBuf,   // Subvolume 2: vector data for streaming
}

impl StreamingBlockchain {
    /// Initialize streaming blockchain with dual subvolumes
    pub async fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let timing_subvol = base_path.join("timing");
        let vector_subvol = base_path.join("vectors");

        // Create base directory
        tokio::fs::create_dir_all(&base_path).await?;

        // Create btrfs subvolumes
        Self::create_subvolume(&timing_subvol).await?;
        Self::create_subvolume(&vector_subvol).await?;

        Ok(Self {
            base_path,
            timing_subvol,
            vector_subvol,
        })
    }

    /// Create btrfs subvolume
    async fn create_subvolume(path: &Path) -> Result<()> {
        if !path.exists() {
            Command::new("btrfs")
                .args(["subvolume", "create"])
                .arg(path)
                .output()
                .await
                .context("Failed to create btrfs subvolume")?;
        }
        Ok(())
    }

    /// Add event from plugin footprint
    pub async fn add_footprint(&self, footprint: PluginFootprint) -> Result<String> {
        let data = serde_json::json!({
            "plugin_id": footprint.plugin_id,
            "operation": footprint.operation,
            "data_hash": footprint.data_hash,
            "metadata": footprint.metadata
        });

        // Use footprint's pre-computed vector features
        let event = BlockEvent {
            timestamp: footprint.timestamp,
            category: footprint.plugin_id.clone(),
            action: footprint.operation.clone(),
            data,
            hash: footprint.content_hash.clone(),
            vector: footprint.vector_features,
        };

        // Write to timing subvolume
        let timing_file = self.timing_subvol.join(format!("{}.json", event.hash));
        let timing_data = serde_json::json!({
            "timestamp": event.timestamp,
            "category": event.category,
            "action": event.action,
            "hash": event.hash,
            "data": event.data,
            "plugin_footprint": true
        });
        tokio::fs::write(&timing_file, serde_json::to_string_pretty(&timing_data)?).await?;

        // Write to vector subvolume
        let vector_file = self.vector_subvol.join(format!("{}.vec", event.hash));
        let vector_data = serde_json::json!({
            "hash": event.hash,
            "vector": event.vector,
            "metadata": {
                "category": event.category,
                "action": event.action,
                "timestamp": event.timestamp,
                "plugin_id": footprint.plugin_id,
                "data_hash": footprint.data_hash
            }
        });
        tokio::fs::write(&vector_file, serde_json::to_string(&vector_data)?).await?;

        // Create snapshots
        self.create_snapshot(&event.hash).await?;

        info!("Plugin footprint added with hash: {}", event.hash);
        Ok(event.hash)
    }

    /// Start footprint receiver
    pub async fn start_footprint_receiver(
        &self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<PluginFootprint>,
    ) {
        info!("Starting plugin footprint receiver");
        
        while let Some(footprint) = receiver.recv().await {
            if let Err(e) = self.add_footprint(footprint).await {
                tracing::error!("Failed to add plugin footprint: {}", e);
            }
        }
    }
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Vectorize data on creation
        let vector = self.vectorize_data(&category, &action, &data);
        
        // Create hash
        let hash_input = format!("{}{}{}{}", timestamp, category, action, data);
        let hash = format!("{:x}", md5::compute(hash_input));

        let event = BlockEvent {
            timestamp,
            category: category.to_string(),
            action: action.to_string(),
            data,
            hash: hash.clone(),
            vector,
        };

        // Write to timing subvolume (creation/timing data)
        let timing_file = self.timing_subvol.join(format!("{}.json", hash));
        let timing_data = serde_json::json!({
            "timestamp": event.timestamp,
            "category": event.category,
            "action": event.action,
            "hash": event.hash,
            "data": event.data
        });
        tokio::fs::write(&timing_file, serde_json::to_string_pretty(&timing_data)?).await?;

        // Write to vector subvolume (vector data for streaming)
        let vector_file = self.vector_subvol.join(format!("{}.vec", hash));
        let vector_data = serde_json::json!({
            "hash": event.hash,
            "vector": event.vector,
            "metadata": {
                "category": event.category,
                "action": event.action,
                "timestamp": event.timestamp
            }
        });
        tokio::fs::write(&vector_file, serde_json::to_string(&vector_data)?).await?;

        // Create snapshots for both subvolumes
        self.create_snapshot(&hash).await?;

        info!("Event added with hash: {}", hash);
        Ok(hash)
    }

    /// Vectorize data on creation/modification
    fn vectorize_data(&self, category: &str, action: &str, data: &serde_json::Value) -> Vec<f32> {
        let mut vector = Vec::with_capacity(128);
        
        // Category embedding (simple hash-based)
        let cat_hash = self.hash_string(category) as f32 / u32::MAX as f32;
        vector.push(cat_hash);
        
        // Action embedding
        let action_hash = self.hash_string(action) as f32 / u32::MAX as f32;
        vector.push(action_hash);
        
        // Data features
        match data {
            serde_json::Value::Object(obj) => {
                // Object size
                vector.push(obj.len() as f32 / 100.0);
                
                // Key features
                for (key, value) in obj.iter().take(10) {
                    let key_hash = self.hash_string(key) as f32 / u32::MAX as f32;
                    vector.push(key_hash);
                    
                    let value_feature = match value {
                        serde_json::Value::String(s) => s.len() as f32 / 1000.0,
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) as f32,
                        serde_json::Value::Bool(b) => if *b { 1.0 } else { 0.0 },
                        _ => 0.5,
                    };
                    vector.push(value_feature);
                }
            }
            serde_json::Value::String(s) => {
                vector.push(s.len() as f32 / 1000.0);
                vector.push(self.hash_string(s) as f32 / u32::MAX as f32);
            }
            serde_json::Value::Number(n) => {
                vector.push(n.as_f64().unwrap_or(0.0) as f32);
            }
            _ => vector.push(0.0),
        }
        
        // Pad to fixed size
        vector.resize(128, 0.0);
        vector
    }

    /// Simple string hash for vectorization
    fn hash_string(&self, s: &str) -> u32 {
        s.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
    }

    /// Create snapshots for both subvolumes
    async fn create_snapshot(&self, block_hash: &str) -> Result<()> {
        let snapshot_dir = self.base_path.join("snapshots");
        tokio::fs::create_dir_all(&snapshot_dir).await?;

        // Snapshot timing subvolume
        let timing_snapshot = snapshot_dir.join(format!("timing-{}", block_hash));
        Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.timing_subvol)
            .arg(&timing_snapshot)
            .output()
            .await
            .context("Failed to create timing snapshot")?;

        // Snapshot vector subvolume  
        let vector_snapshot = snapshot_dir.join(format!("vectors-{}", block_hash));
        Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.vector_subvol)
            .arg(&vector_snapshot)
            .output()
            .await
            .context("Failed to create vector snapshot")?;

        debug!("Created snapshots for block: {}", block_hash);
        Ok(())
    }

    /// Stream vector subvolume to remote
    pub async fn stream_vectors(&self, block_hash: &str, remote: &str) -> Result<()> {
        let vector_snapshot = self.base_path.join("snapshots").join(format!("vectors-{}", block_hash));
        
        info!("Streaming vectors for block {} to {}", block_hash, remote);
        
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                "btrfs send {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
                vector_snapshot.display(),
                remote
            ))
            .output()
            .await
            .context("Failed to stream vectors")?;

        if !output.status.success() {
            anyhow::bail!("Stream failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Stream incremental vectors
    pub async fn stream_vectors_incremental(&self, prev_hash: &str, current_hash: &str, remote: &str) -> Result<()> {
        let prev_snapshot = self.base_path.join("snapshots").join(format!("vectors-{}", prev_hash));
        let current_snapshot = self.base_path.join("snapshots").join(format!("vectors-{}", current_hash));
        
        info!("Streaming incremental vectors {} -> {} to {}", prev_hash, current_hash, remote);
        
        let output = Command::new("bash")
            .arg("-c")
            .arg(format!(
                "btrfs send -p {} {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
                prev_snapshot.display(),
                current_snapshot.display(),
                remote
            ))
            .output()
            .await
            .context("Failed to stream incremental vectors")?;

        if !output.status.success() {
            anyhow::bail!("Incremental stream failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Stream to multiple replicas
    pub async fn stream_to_replicas(&self, block_hash: &str, replicas: &[String]) -> Result<()> {
        let vector_snapshot = self.base_path.join("snapshots").join(format!("vectors-{}", block_hash));
        
        let mut tee_args = Vec::new();
        for replica in replicas {
            tee_args.push(format!(">(ssh {} 'btrfs receive /var/lib/blockchain/vectors/')", replica));
        }
        
        let cmd = format!(
            "btrfs send {} | tee {} > /dev/null",
            vector_snapshot.display(),
            tee_args.join(" ")
        );
        
        info!("Streaming to {} replicas", replicas.len());
        
        let output = Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .context("Failed to stream to replicas")?;

        if !output.status.success() {
            anyhow::bail!("Multi-replica stream failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Query vectors by similarity
    pub async fn query_similar(&self, query_vector: &[f32], limit: usize) -> Result<Vec<BlockEvent>> {
        let mut events = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.vector_subvol).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().map_or(false, |ext| ext == "vec") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                let vector_data: serde_json::Value = serde_json::from_str(&content)?;
                
                if let Some(vector) = vector_data["vector"].as_array() {
                    let vec: Vec<f32> = vector.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();
                    
                    let similarity = self.cosine_similarity(query_vector, &vec);
                    
                    if similarity > 0.8 { // Similarity threshold
                        // Load full event from timing subvolume
                        let hash = vector_data["hash"].as_str().unwrap();
                        let timing_file = self.timing_subvol.join(format!("{}.json", hash));
                        let timing_content = tokio::fs::read_to_string(timing_file).await?;
                        let timing_data: serde_json::Value = serde_json::from_str(&timing_content)?;
                        
                        events.push(BlockEvent {
                            timestamp: timing_data["timestamp"].as_u64().unwrap(),
                            category: timing_data["category"].as_str().unwrap().to_string(),
                            action: timing_data["action"].as_str().unwrap().to_string(),
                            data: timing_data["data"].clone(),
                            hash: hash.to_string(),
                            vector: vec,
                        });
                    }
                }
            }
        }
        
        events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        events.truncate(limit);
        Ok(events)
    }

    /// Calculate cosine similarity between vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Get latest block hash
    pub async fn get_latest_block(&self) -> Result<Option<String>> {
        let snapshot_dir = self.base_path.join("snapshots");
        if !snapshot_dir.exists() {
            return Ok(None);
        }

        let mut entries = tokio::fs::read_dir(&snapshot_dir).await?;
        let mut latest_time = 0u64;
        let mut latest_hash = None;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("vectors-") {
                let hash = name.strip_prefix("vectors-").unwrap();
                // Extract timestamp from hash or use file modification time
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                            let time = duration.as_secs();
                            if time > latest_time {
                                latest_time = time;
                                latest_hash = Some(hash.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(latest_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_streaming_blockchain() {
        let temp_dir = TempDir::new().unwrap();
        let blockchain = StreamingBlockchain::new(temp_dir.path()).await.unwrap();

        let data = serde_json::json!({
            "interface": "eth0",
            "action": "created",
            "ip": "192.168.1.100"
        });

        let hash = blockchain.add_event("network", "interface_created", data).await.unwrap();
        assert!(!hash.is_empty());

        let latest = blockchain.get_latest_block().await.unwrap();
        assert_eq!(latest, Some(hash));
    }
}
