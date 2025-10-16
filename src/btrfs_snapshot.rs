//! Btrfs Snapshot Interceptor - Ephemeral snapshots for data transformation
//!
//! Creates instant snapshots, transforms data, then immediately deletes snapshot

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// RAII guard that deletes snapshot on drop
pub struct SnapshotGuard {
    path: PathBuf,
}

impl SnapshotGuard {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        // Synchronous delete on drop - handle path conversion errors gracefully
        if let Some(path_str) = self.path.to_str() {
            let _ = std::process::Command::new("btrfs")
                .args(["subvolume", "delete", path_str])
                .output();
        } else {
            eprintln!("Warning: Failed to convert snapshot path to string for deletion");
        }
    }
}

/// Btrfs snapshot manager
pub struct BtrfsSnapshot {
    base_path: PathBuf,
}

impl BtrfsSnapshot {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }
    
    /// Create ephemeral snapshot (auto-deleted via RAII)
    ///
    /// # Errors
    /// Returns error if snapshot creation fails or path conversion fails
    ///
    /// # Panics
    /// Panics if path cannot be converted to string
    pub async fn create_ephemeral(&self, source: &str) -> Result<SnapshotGuard> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        
        let snapshot_name = format!("@ephemeral-{timestamp}");
        let snapshot_path = self.base_path.join(&snapshot_name);
        let source_path = self.base_path.join(source);
        
        // Create instant snapshot - validate paths first
        let source_str = source_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Source path contains invalid UTF-8: {:?}", source_path))?;
        let snapshot_str = snapshot_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Snapshot path contains invalid UTF-8: {:?}", snapshot_path))?;

        let output = Command::new("btrfs")
            .args([
                "subvolume",
                "snapshot",
                "-r", // read-only
                source_str,
                snapshot_str,
            ])
            .output()
            .await
            .context("Failed to create snapshot")?;
        
        if !output.status.success() {
            anyhow::bail!(
                "Snapshot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        Ok(SnapshotGuard {
            path: snapshot_path,
        })
    }
    
    /// Transform data from snapshot to normalized format
    ///
    /// # Errors
    /// Returns error if snapshot creation or transformation fails
    pub async fn transform_and_cleanup<F, T>(&self, source: &str, transform_fn: F) -> Result<T>
    where
        F: FnOnce(&Path) -> Result<T>,
    {
        // Create ephemeral snapshot
        let snapshot = self.create_ephemeral(source).await?;
        
        // Transform data (snapshot auto-deleted after this scope)
        transform_fn(snapshot.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ephemeral_snapshot() {
        // Skip this test in CI/testing environments without btrfs setup
        if !std::path::Path::new("/tmp/test-snapshots/@current").exists() {
            println!("Skipping btrfs snapshot test - no test btrfs subvolumes available");
            return;
        }

        let manager = BtrfsSnapshot::new("/tmp/test-snapshots");

        // Snapshot is created and immediately deleted after transform
        let result = manager
            .transform_and_cleanup("@current", |path| {
                // Do transformation here
                assert!(path.exists());
                Ok(42)
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
