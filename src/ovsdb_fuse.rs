//! OVSDB FUSE Filesystem - Dual-path UUID and name mapping
//!
//! Exposes OVSDB as a filesystem with two views:
//! - `/by-uuid/` - Canonical OVSDB representation with UUIDs
//! - `/by-name/` - Human-readable names (symlinks to by-uuid)
//! - `/aliases/` - User-defined aliases

use anyhow::{Context, Result};
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request, FUSE_ROOT_ID,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

const TTL: Duration = Duration::from_secs(1);

/// Inode numbers for virtual directories
const INODE_ROOT: u64 = FUSE_ROOT_ID;
const INODE_BY_UUID: u64 = 2;
const INODE_BY_NAME: u64 = 3;
const INODE_ALIASES: u64 = 4;
const INODE_BY_UUID_BRIDGES: u64 = 5;
const INODE_BY_UUID_PORTS: u64 = 6;
const INODE_BY_NAME_BRIDGES: u64 = 7;
const INODE_BY_NAME_PORTS: u64 = 8;

/// OVSDB entity (bridge, port, interface)
#[derive(Debug, Clone)]
struct OvsdbEntity {
    uuid: String,
    name: String,
    entity_type: EntityType,
    attributes: HashMap<String, String>,
    relationships: HashMap<String, Vec<String>>, // e.g., "ports" -> [uuid1, uuid2]
}

#[derive(Debug, Clone, PartialEq)]
enum EntityType {
    Bridge,
    Port,
    Interface,
}

/// In-memory cache of OVSDB state
#[derive(Debug, Default)]
struct OvsdbCache {
    bridges: HashMap<String, OvsdbEntity>,      // uuid -> entity
    ports: HashMap<String, OvsdbEntity>,        // uuid -> entity
    interfaces: HashMap<String, OvsdbEntity>,   // uuid -> entity
    
    // Reverse lookups
    bridge_names: HashMap<String, String>,      // name -> uuid
    port_names: HashMap<String, String>,        // name -> uuid
    interface_names: HashMap<String, String>,   // name -> uuid
    
    // Inode mappings
    next_inode: u64,
    inode_to_path: HashMap<u64, String>,
    path_to_inode: HashMap<String, u64>,
}

impl OvsdbCache {
    fn new() -> Self {
        let mut cache = Self {
            next_inode: 100, // Start after reserved inodes
            ..Default::default()
        };
        
        // Register virtual directories
        cache.register_path(INODE_ROOT, "/");
        cache.register_path(INODE_BY_UUID, "/by-uuid");
        cache.register_path(INODE_BY_NAME, "/by-name");
        cache.register_path(INODE_ALIASES, "/aliases");
        cache.register_path(INODE_BY_UUID_BRIDGES, "/by-uuid/bridges");
        cache.register_path(INODE_BY_UUID_PORTS, "/by-uuid/ports");
        cache.register_path(INODE_BY_NAME_BRIDGES, "/by-name/bridges");
        cache.register_path(INODE_BY_NAME_PORTS, "/by-name/ports");
        
        cache
    }
    
    fn register_path(&mut self, inode: u64, path: &str) {
        self.inode_to_path.insert(inode, path.to_string());
        self.path_to_inode.insert(path.to_string(), inode);
    }
    
    fn allocate_inode(&mut self, path: &str) -> u64 {
        if let Some(&inode) = self.path_to_inode.get(path) {
            return inode;
        }
        
        let inode = self.next_inode;
        self.next_inode += 1;
        self.register_path(inode, path);
        inode
    }
    
    fn get_inode(&self, path: &str) -> Option<u64> {
        self.path_to_inode.get(path).copied()
    }
    
    fn get_path(&self, inode: u64) -> Option<&str> {
        self.inode_to_path.get(&inode).map(String::as_str)
    }
    
    /// Add bridge to cache
    fn add_bridge(&mut self, uuid: &str, name: &str, entity: OvsdbEntity) {
        self.bridge_names.insert(name.to_string(), uuid.to_string());
        self.bridges.insert(uuid.to_string(), entity);
        
        // Allocate inodes for paths
        self.allocate_inode(&format!("/by-uuid/bridges/{uuid}"));
        self.allocate_inode(&format!("/by-name/bridges/{name}"));
    }
    
    /// Get bridge by name
    fn get_bridge_by_name(&self, name: &str) -> Option<&OvsdbEntity> {
        self.bridge_names.get(name)
            .and_then(|uuid| self.bridges.get(uuid))
    }
    
    /// Get bridge by UUID
    fn get_bridge_by_uuid(&self, uuid: &str) -> Option<&OvsdbEntity> {
        self.bridges.get(uuid)
    }
}

/// OVSDB FUSE filesystem
pub struct OvsdbFuse {
    cache: Arc<RwLock<OvsdbCache>>,
}

impl OvsdbFuse {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(OvsdbCache::new())),
        }
    }
}

impl Default for OvsdbFuse {
    fn default() -> Self {
        Self::new()
    }
}

impl OvsdbFuse {
    /// Initialize with sample data (for testing)
    ///
    /// # Panics
    /// Panics if the cache lock is poisoned
    pub fn init_sample_data(&self) {
        let mut cache = self.cache.write().unwrap();

        // Add sample bridge
        let bridge = OvsdbEntity {
            uuid: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "ovsbr0".to_string(),
            entity_type: EntityType::Bridge,
            attributes: [
                ("datapath_type".to_string(), "system".to_string()),
                ("fail_mode".to_string(), "standalone".to_string()),
            ].into(),
            relationships: HashMap::new(),
        };

        cache.add_bridge(
            "550e8400-e29b-41d4-a716-446655440000",
            "ovsbr0",
            bridge,
        );
    }

    fn get_attr(inode: u64, file_type: FileType) -> FileAttr {
        FileAttr {
            ino: inode,
            size: 0,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: file_type,
            perm: if file_type == FileType::Directory { 0o755 } else { 0o644 },
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }
}

impl Filesystem for OvsdbFuse {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let cache = self.cache.read().unwrap();
        let name_str = name.to_str().unwrap();
        
        let Some(parent_path) = cache.get_path(parent) else {
            reply.error(libc::ENOENT);
            return;
        };
        
        let full_path = format!("{parent_path}/{name_str}");
        
        match full_path.as_str() {
            "/by-uuid" | "/by-name" | "/aliases" 
            | "/by-uuid/bridges" | "/by-uuid/ports" | "/by-name/bridges" | "/by-name/ports" => {
                let inode = cache.get_inode(&full_path).unwrap();
                reply.entry(&TTL, &Self::get_attr(inode, FileType::Directory), 0);
            }
            
            path if path.starts_with("/by-name/bridges/") => {
                // Lookup bridge by name (returns symlink)
                if cache.get_bridge_by_name(name_str).is_some() {
                    let inode = cache.get_inode(&full_path)
                        .unwrap_or_else(|| {
                            drop(cache);
                            self.cache.write().unwrap().allocate_inode(&full_path)
                        });
                    reply.entry(&TTL, &Self::get_attr(inode, FileType::Symlink), 0);
                } else {
                    reply.error(libc::ENOENT);
                }
            }
            
            path if path.starts_with("/by-uuid/bridges/") => {
                // Lookup bridge by UUID (returns directory)
                if cache.get_bridge_by_uuid(name_str).is_some() {
                    let inode = cache.get_inode(&full_path)
                        .unwrap_or_else(|| {
                            drop(cache);
                            self.cache.write().unwrap().allocate_inode(&full_path)
                        });
                    reply.entry(&TTL, &Self::get_attr(inode, FileType::Directory), 0);
                } else {
                    reply.error(libc::ENOENT);
                }
            }
            
            _ => reply.error(libc::ENOENT),
        }
    }
    
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        let cache = self.cache.read().unwrap();
        
        let file_type = match ino {
            INODE_ROOT | INODE_BY_UUID | INODE_BY_NAME | INODE_ALIASES |
            INODE_BY_UUID_BRIDGES | INODE_BY_UUID_PORTS |
            INODE_BY_NAME_BRIDGES | INODE_BY_NAME_PORTS => FileType::Directory,
            
            _ => {
                if let Some(path) = cache.get_path(ino) {
                    if path.starts_with("/by-name/") {
                        FileType::Symlink
                    } else if path.starts_with("/by-uuid/") {
                        FileType::Directory
                    } else {
                        FileType::RegularFile
                    }
                } else {
                    reply.error(libc::ENOENT);
                    return;
                }
            }
        };
        
        reply.attr(&TTL, &Self::get_attr(ino, file_type));
    }
    
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let cache = self.cache.read().unwrap();
        
        let entries: Vec<(u64, FileType, String)> = match ino {
            INODE_ROOT => vec![
                (INODE_BY_UUID, FileType::Directory, "by-uuid".to_string()),
                (INODE_BY_NAME, FileType::Directory, "by-name".to_string()),
                (INODE_ALIASES, FileType::Directory, "aliases".to_string()),
            ],
            
            INODE_BY_UUID => vec![
                (INODE_BY_UUID_BRIDGES, FileType::Directory, "bridges".to_string()),
                (INODE_BY_UUID_PORTS, FileType::Directory, "ports".to_string()),
            ],
            
            INODE_BY_NAME => vec![
                (INODE_BY_NAME_BRIDGES, FileType::Directory, "bridges".to_string()),
                (INODE_BY_NAME_PORTS, FileType::Directory, "ports".to_string()),
            ],
            
            INODE_BY_UUID_BRIDGES => {
                cache.bridges.keys()
                    .map(|uuid| {
                        let path = format!("/by-uuid/bridges/{uuid}");
                        let inode = cache.get_inode(&path).unwrap();
                        (inode, FileType::Directory, uuid.clone())
                    })
                    .collect()
            }
            
            INODE_BY_NAME_BRIDGES => {
                cache.bridge_names.keys()
                    .map(|name| {
                        let path = format!("/by-name/bridges/{name}");
                        let inode = cache.get_inode(&path).unwrap();
                        (inode, FileType::Symlink, name.clone())
                    })
                    .collect()
            }
            
            _ => vec![],
        };
        
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        for (i, (inode, file_type, name)) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(inode, (i + 1) as i64, file_type, name) {
                break;
            }
        }
        
        reply.ok();
    }
    
    fn readlink(&mut self, _req: &Request, ino: u64, reply: ReplyData) {
        let cache = self.cache.read().unwrap();
        
        if let Some(path) = cache.get_path(ino) {
            if path.starts_with("/by-name/bridges/") {
                let name = path.strip_prefix("/by-name/bridges/").unwrap();
                if let Some(entity) = cache.get_bridge_by_name(name) {
                    let target = format!("../../by-uuid/bridges/{}", entity.uuid);
                    reply.data(target.as_bytes());
                    return;
                }
            }
        }
        
        reply.error(libc::ENOENT);
    }
}
