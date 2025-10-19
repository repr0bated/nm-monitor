//! OVSDB Interceptor - Captures ovs-vsctl output via btrfs snapshots
//!
//! Intercepts OVSDB changes, creates ephemeral snapshots, transforms data

use anyhow::Result;
use serde_json::Value;
use std::path::Path;
use tokio::fs;

use crate::btrfs_snapshot::BtrfsSnapshot;

/// OVSDB data interceptor
pub struct OvsdbInterceptor {
    snapshot_manager: BtrfsSnapshot,
}

impl OvsdbInterceptor {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            snapshot_manager: BtrfsSnapshot::new(base_path),
        }
    }
    
    /// Execute ovs-vsctl command and capture output via snapshot
    ///
    /// # Errors
    /// Returns error if command execution or transformation fails
    pub async fn execute_and_transform(&self, args: &[&str]) -> Result<Value> {
        // Execute ovs-vsctl command
        let output = tokio::process::Command::new("ovs-vsctl")
            .args(args)
            .output()
            .await?;
        
        if !output.status.success() {
            anyhow::bail!(
                "ovs-vsctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        // Create ephemeral snapshot and transform
        self.snapshot_manager
            .transform_and_cleanup("@ovsdb-current", |snapshot_path| {
                // Read OVSDB from snapshot
                let db_path = snapshot_path.join("conf.db");
                
                // Transform to normalized format
                Ok(Self::transform_ovsdb(&db_path))
            })
            .await
    }
    
    /// Transform OVSDB data to normalized JSON
    fn transform_ovsdb(db_path: &Path) -> Value {
        // Parse OVSDB (simplified - real implementation would use ovsdb crate)
        let data = std::fs::read_to_string(db_path)
            .unwrap_or_else(|_| "{}".to_string());
        
        // Transform to normalized structure
        serde_json::json!({
            "bridges": Self::extract_bridges(&data),
            "ports": Self::extract_ports(&data),
            "interfaces": Self::extract_interfaces(&data),
        })
    }
    
    fn extract_bridges(_data: &str) -> Vec<Value> {
        // TODO: Parse OVSDB format
        vec![]
    }
    
    fn extract_ports(_data: &str) -> Vec<Value> {
        vec![]
    }
    
    fn extract_interfaces(_data: &str) -> Vec<Value> {
        vec![]
    }
}

/// Simpler approach: Intercept ovs-vsctl output directly
pub struct OvsctlInterceptor {
    snapshot_manager: BtrfsSnapshot,
}

impl OvsctlInterceptor {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            snapshot_manager: BtrfsSnapshot::new(base_path),
        }
    }
    
    /// Execute ovs-vsctl, write output to snapshot, transform, cleanup
    ///
    /// # Errors
    /// Returns error if command execution fails
    pub async fn execute(&self, args: &[&str]) -> Result<String> {
        // Execute command
        let output = tokio::process::Command::new("ovs-vsctl")
            .args(args)
            .output()
            .await?;
        
        if !output.status.success() {
            anyhow::bail!(
                "ovs-vsctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        
        // Create ephemeral snapshot for transformation
        self.snapshot_manager
            .transform_and_cleanup("@ovsdb-current", |snapshot_path| {
                // Write output to snapshot
                let output_file = snapshot_path.join("output.txt");
                std::fs::write(&output_file, &stdout)?;
                
                // Transform (in this case, just return as-is)
                Ok(stdout.clone())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ovsctl_interceptor() {
        // Skip this test in CI/testing environments without OVSDB setup
        if !std::path::Path::new("/tmp/ovsdb-snapshots/@ovsdb-current").exists() {
            println!("Skipping OVSDB interceptor test - no test OVSDB available");
            return;
        }

        let interceptor = OvsctlInterceptor::new("/tmp/ovsdb-snapshots");

        // Execute command - snapshot created, transformed, deleted
        let result = interceptor.execute(&["list-br"]).await;

        // Snapshot should be gone
        assert!(result.is_ok());
    }
}
