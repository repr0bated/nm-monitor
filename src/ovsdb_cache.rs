//! OVSDB Cache - Maintains dual-path UUID/name mapping
//!
//! Creates a simple directory structure that D-Bus can read:
//! /var/lib/ovsdb-cache/
//! ├── by-uuid/bridges/<uuid>/name
//! ├── by-name/bridges/<name> -> ../../by-uuid/bridges/<uuid>
//!
//! This avoids FUSE complexity while providing the same dual-path benefits

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

const CACHE_DIR: &str = "/var/lib/ovsdb-cache";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsdbEntity {
    pub uuid: String,
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub relationships: HashMap<String, Vec<String>>,
}

pub struct OvsdbCache {
    cache_dir: PathBuf,
}

impl OvsdbCache {
    pub fn new() -> Self {
        Self {
            cache_dir: PathBuf::from(CACHE_DIR),
        }
    }
    
    /// Initialize cache directory structure
    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).await?;
        fs::create_dir_all(self.cache_dir.join("by-uuid/bridges")).await?;
        fs::create_dir_all(self.cache_dir.join("by-uuid/ports")).await?;
        fs::create_dir_all(self.cache_dir.join("by-name/bridges")).await?;
        fs::create_dir_all(self.cache_dir.join("by-name/ports")).await?;
        fs::create_dir_all(self.cache_dir.join("aliases")).await?;
        Ok(())
    }
    
    /// Add bridge to cache (creates both UUID dir and name symlink)
    pub async fn add_bridge(&self, entity: OvsdbEntity) -> Result<()> {
        let uuid_dir = self.cache_dir.join(format!("by-uuid/bridges/{}", entity.uuid));
        fs::create_dir_all(&uuid_dir).await?;
        
        // Write attributes as files
        fs::write(uuid_dir.join("name"), &entity.name).await?;
        fs::write(uuid_dir.join("_uuid"), &entity.uuid).await?;
        
        for (key, value) in &entity.attributes {
            fs::write(uuid_dir.join(key), value).await?;
        }
        
        // Create name symlink
        let name_link = self.cache_dir.join(format!("by-name/bridges/{}", entity.name));
        let target = format!("../../by-uuid/bridges/{}", entity.uuid);
        
        // Remove existing symlink if present
        let _ = fs::remove_file(&name_link).await;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            tokio::task::spawn_blocking(move || {
                symlink(&target, &name_link)
            }).await??;
        }
        
        Ok(())
    }
    
    /// List bridges by name
    pub async fn list_bridges_by_name(&self) -> Result<Vec<String>> {
        let dir = self.cache_dir.join("by-name/bridges");
        let mut entries = fs::read_dir(dir).await?;
        let mut names = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
        
        Ok(names)
    }
    
    /// List bridges by UUID
    pub async fn list_bridges_by_uuid(&self) -> Result<Vec<String>> {
        let dir = self.cache_dir.join("by-uuid/bridges");
        let mut entries = fs::read_dir(dir).await?;
        let mut uuids = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            uuids.push(entry.file_name().to_string_lossy().to_string());
        }
        
        Ok(uuids)
    }
    
    /// Get bridge UUID from name (follow symlink)
    pub async fn get_bridge_uuid(&self, name: &str) -> Result<String> {
        let link = self.cache_dir.join(format!("by-name/bridges/{}", name));
        let target = fs::read_link(link).await?;
        
        // Extract UUID from path: ../../by-uuid/bridges/<uuid>
        let uuid = target.file_name()
            .context("Invalid symlink target")?
            .to_string_lossy()
            .to_string();
        
        Ok(uuid)
    }
    
    /// Get bridge name from UUID
    pub async fn get_bridge_name(&self, uuid: &str) -> Result<String> {
        let name_file = self.cache_dir.join(format!("by-uuid/bridges/{}/name", uuid));
        let name = fs::read_to_string(name_file).await?;
        Ok(name)
    }
    
    /// Get bridge attribute
    pub async fn get_bridge_attr(&self, uuid: &str, attr: &str) -> Result<String> {
        let attr_file = self.cache_dir.join(format!("by-uuid/bridges/{}/{}", uuid, attr));
        let value = fs::read_to_string(attr_file).await?;
        Ok(value)
    }
    
    /// Remove bridge from cache
    pub async fn remove_bridge(&self, uuid: &str) -> Result<()> {
        // Get name first
        let name = self.get_bridge_name(uuid).await?;
        
        // Remove UUID directory
        let uuid_dir = self.cache_dir.join(format!("by-uuid/bridges/{}", uuid));
        fs::remove_dir_all(uuid_dir).await?;
        
        // Remove name symlink
        let name_link = self.cache_dir.join(format!("by-name/bridges/{}", name));
        fs::remove_file(name_link).await?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cache_operations() {
        let cache = OvsdbCache::new();
        cache.init().await.unwrap();
        
        let entity = OvsdbEntity {
            uuid: "test-uuid-123".to_string(),
            name: "test-bridge".to_string(),
            attributes: [
                ("datapath_type".to_string(), "system".to_string()),
            ].into(),
            relationships: HashMap::new(),
        };
        
        cache.add_bridge(entity).await.unwrap();
        
        let names = cache.list_bridges_by_name().await.unwrap();
        assert!(names.contains(&"test-bridge".to_string()));
        
        let uuid = cache.get_bridge_uuid("test-bridge").await.unwrap();
        assert_eq!(uuid, "test-uuid-123");
    }
}
