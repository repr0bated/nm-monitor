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
const INODE_BY_UUID_INTERFACES: u64 = 9;
const INODE_BY_NAME_BRIDGES: u64 = 7;
const INODE_BY_NAME_PORTS: u64 = 8;
const INODE_BY_NAME_INTERFACES: u64 = 10;

/// OVSDB entity (bridge, port, interface) with complete attribute introspection
#[derive(Debug, Clone)]
struct OvsdbEntity {
    uuid: String,
    name: String,
    entity_type: EntityType,
    attributes: HashMap<String, serde_json::Value>, // Full JSON attribute values
    relationships: HashMap<String, Vec<String>>, // e.g., "ports" -> [uuid1, uuid2]
    raw_json: serde_json::Value, // Complete JSON representation
}

#[derive(Debug, Clone, PartialEq)]
enum EntityType {
    Bridge,
    Port,
    Interface,
}

/// In-memory cache of OVSDB state with complete introspection
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
        cache.register_path(INODE_BY_UUID_INTERFACES, "/by-uuid/interfaces");
        cache.register_path(INODE_BY_NAME_BRIDGES, "/by-name/bridges");
        cache.register_path(INODE_BY_NAME_PORTS, "/by-name/ports");
        cache.register_path(INODE_BY_NAME_INTERFACES, "/by-name/interfaces");
        
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

    /// Add port to cache
    fn add_port(&mut self, uuid: &str, name: &str, entity: OvsdbEntity) {
        self.port_names.insert(name.to_string(), uuid.to_string());
        self.ports.insert(uuid.to_string(), entity);

        // Allocate inodes for paths
        self.allocate_inode(&format!("/by-uuid/ports/{uuid}"));
        self.allocate_inode(&format!("/by-name/ports/{name}"));
    }

    /// Add interface to cache
    fn add_interface(&mut self, uuid: &str, name: &str, entity: OvsdbEntity) {
        self.interface_names.insert(name.to_string(), uuid.to_string());
        self.interfaces.insert(uuid.to_string(), entity);

        // Allocate inodes for paths
        self.allocate_inode(&format!("/by-uuid/interfaces/{uuid}"));
        self.allocate_inode(&format!("/by-name/interfaces/{name}"));
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

    /// Get port by name
    fn get_port_by_name(&self, name: &str) -> Option<&OvsdbEntity> {
        self.port_names.get(name)
            .and_then(|uuid| self.ports.get(uuid))
    }

    /// Get port by UUID
    fn get_port_by_uuid(&self, uuid: &str) -> Option<&OvsdbEntity> {
        self.ports.get(uuid)
    }

    /// Get interface by name
    fn get_interface_by_name(&self, name: &str) -> Option<&OvsdbEntity> {
        self.interface_names.get(name)
            .and_then(|uuid| self.interfaces.get(uuid))
    }

    /// Get interface by UUID
    fn get_interface_by_uuid(&self, uuid: &str) -> Option<&OvsdbEntity> {
        self.interfaces.get(uuid)
    }

    /// Load real OVSDB data from the system
    fn load_real_data(&mut self) -> Result<()> {
        self.load_bridges()?;
        self.load_ports()?;
        self.load_interfaces()?;
        Ok(())
    }

    /// Load all bridges from OVSDB
    fn load_bridges(&mut self) -> Result<()> {
        let bridge_list = crate::ovs_introspect::list_bridges_json()?;

        if let Some(bridges) = bridge_list.as_array() {
            for bridge_name in bridges {
                if let Some(name) = bridge_name.as_str() {
                    if let Ok(bridge_info) = crate::ovs_introspect::bridge_info_json(name) {
                        self.add_bridge_from_json(name, bridge_info)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Load all ports from OVSDB
    fn load_ports(&mut self) -> Result<()> {
        // For each bridge, get its ports
        for (bridge_uuid, bridge_entity) in &self.bridges.clone() {
            if let Ok(ports_json) = crate::ovs_introspect::bridge_ports_json(&bridge_entity.name) {
                if let Some(ports) = ports_json.as_array() {
                    for port_name in ports {
                        if let Some(name) = port_name.as_str() {
                            if let Ok(port_info) = crate::ovs_introspect::port_info_json(name) {
                                self.add_port_from_json(name, port_info)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load all interfaces from OVSDB
    fn load_interfaces(&mut self) -> Result<()> {
        let interfaces_json = crate::ovs_introspect::list_interfaces_json()?;

        if let Some(interfaces) = interfaces_json.as_array() {
            for interface_name in interfaces {
                if let Some(name) = interface_name.as_str() {
                    if let Ok(iface_info) = crate::ovs_introspect::interface_info_json(name) {
                        self.add_interface_from_json(name, iface_info)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Add bridge from JSON data
    fn add_bridge_from_json(&mut self, name: &str, json_data: serde_json::Value) -> Result<()> {
        // Clone json_data to avoid borrowing issues
        let json_clone = json_data.clone();
        if let Some(uuid) = json_clone.get("_uuid").and_then(|v| v.as_array()).and_then(|arr| arr.get(1)).and_then(|v| v.as_str()) {
            let mut attributes = HashMap::new();
            let mut relationships = HashMap::new();

            // Extract all attributes from JSON
            if let Some(data) = json_data.as_object() {
                for (key, value) in data {
                    if key == "_uuid" || key == "_version" {
                        continue; // Skip metadata
                    }
                    attributes.insert(key.clone(), value.clone());
                }
            }

            // Extract ports relationship
            if let Some(ports) = attributes.get("ports").and_then(|v| v.as_array()) {
                let port_uuids: Vec<String> = ports.iter()
                    .filter_map(|p| p.as_array())
                    .filter_map(|arr| arr.get(1))
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                relationships.insert("ports".to_string(), port_uuids);
            }

            let entity = OvsdbEntity {
                uuid: uuid.to_string(),
                name: name.to_string(),
                entity_type: EntityType::Bridge,
                attributes,
                relationships,
                raw_json: json_data,
            };

            self.add_bridge(uuid, name, entity);
        }

        Ok(())
    }

    /// Add port from JSON data
    fn add_port_from_json(&mut self, name: &str, json_data: serde_json::Value) -> Result<()> {
        // Clone json_data to avoid borrowing issues
        let json_clone = json_data.clone();
        if let Some(uuid) = json_clone.get("_uuid").and_then(|v| v.as_array()).and_then(|arr| arr.get(1)).and_then(|v| v.as_str()) {
            let mut attributes = HashMap::new();
            let mut relationships = HashMap::new();

            // Extract all attributes from JSON
            if let Some(data) = json_data.as_object() {
                for (key, value) in data {
                    if key == "_uuid" || key == "_version" {
                        continue; // Skip metadata
                    }
                    attributes.insert(key.clone(), value.clone());
                }
            }

            // Extract interfaces relationship
            if let Some(interfaces) = attributes.get("interfaces").and_then(|v| v.as_array()) {
                let iface_uuids: Vec<String> = interfaces.iter()
                    .filter_map(|p| p.as_array())
                    .filter_map(|arr| arr.get(1))
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                relationships.insert("interfaces".to_string(), iface_uuids);
            }

            let entity = OvsdbEntity {
                uuid: uuid.to_string(),
                name: name.to_string(),
                entity_type: EntityType::Port,
                attributes,
                relationships,
                raw_json: json_data,
            };

            self.add_port(uuid, name, entity);
        }

        Ok(())
    }

    /// Add interface from JSON data
    fn add_interface_from_json(&mut self, name: &str, json_data: serde_json::Value) -> Result<()> {
        // Clone json_data to avoid borrowing issues
        let json_clone = json_data.clone();
        if let Some(uuid) = json_clone.get("_uuid").and_then(|v| v.as_array()).and_then(|arr| arr.get(1)).and_then(|v| v.as_str()) {
            let mut attributes = HashMap::new();
            let relationships = HashMap::new();

            // Extract all attributes from JSON
            if let Some(data) = json_data.as_object() {
                for (key, value) in data {
                    if key == "_uuid" || key == "_version" {
                        continue; // Skip metadata
                    }
                    attributes.insert(key.clone(), value.clone());
                }
            }

            let entity = OvsdbEntity {
                uuid: uuid.to_string(),
                name: name.to_string(),
                entity_type: EntityType::Interface,
                attributes,
                relationships,
                raw_json: json_data,
            };

            self.add_interface(uuid, name, entity);
        }

        Ok(())
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
    /// Load real OVSDB data from the system
    pub fn load_real_data(&self) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.load_real_data()
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
            | "/by-uuid/bridges" | "/by-uuid/ports" | "/by-uuid/interfaces"
            | "/by-name/bridges" | "/by-name/ports" | "/by-name/interfaces" => {
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

            path if path.starts_with("/by-name/ports/") => {
                // Lookup port by name (returns symlink)
                if cache.get_port_by_name(name_str).is_some() {
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

            path if path.starts_with("/by-uuid/ports/") => {
                // Lookup port by UUID (returns directory)
                if cache.get_port_by_uuid(name_str).is_some() {
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

            path if path.starts_with("/by-name/interfaces/") => {
                // Lookup interface by name (returns symlink)
                if cache.get_interface_by_name(name_str).is_some() {
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

            path if path.starts_with("/by-uuid/interfaces/") => {
                // Lookup interface by UUID (returns directory)
                if cache.get_interface_by_uuid(name_str).is_some() {
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
            INODE_BY_UUID_BRIDGES | INODE_BY_UUID_PORTS | INODE_BY_UUID_INTERFACES |
            INODE_BY_NAME_BRIDGES | INODE_BY_NAME_PORTS | INODE_BY_NAME_INTERFACES => FileType::Directory,
            
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
                (INODE_BY_UUID_INTERFACES, FileType::Directory, "interfaces".to_string()),
            ],

            INODE_BY_NAME => vec![
                (INODE_BY_NAME_BRIDGES, FileType::Directory, "bridges".to_string()),
                (INODE_BY_NAME_PORTS, FileType::Directory, "ports".to_string()),
                (INODE_BY_NAME_INTERFACES, FileType::Directory, "interfaces".to_string()),
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

            INODE_BY_UUID_PORTS => {
                cache.ports.keys()
                    .map(|uuid| {
                        let path = format!("/by-uuid/ports/{uuid}");
                        let inode = cache.get_inode(&path).unwrap();
                        (inode, FileType::Directory, uuid.clone())
                    })
                    .collect()
            }

            INODE_BY_NAME_PORTS => {
                cache.port_names.keys()
                    .map(|name| {
                        let path = format!("/by-name/ports/{name}");
                        let inode = cache.get_inode(&path).unwrap();
                        (inode, FileType::Symlink, name.clone())
                    })
                    .collect()
            }

            INODE_BY_UUID_INTERFACES => {
                cache.interfaces.keys()
                    .map(|uuid| {
                        let path = format!("/by-uuid/interfaces/{uuid}");
                        let inode = cache.get_inode(&path).unwrap();
                        (inode, FileType::Directory, uuid.clone())
                    })
                    .collect()
            }

            INODE_BY_NAME_INTERFACES => {
                cache.interface_names.keys()
                    .map(|name| {
                        let path = format!("/by-name/interfaces/{name}");
                        let inode = cache.get_inode(&path).unwrap();
                        (inode, FileType::Symlink, name.clone())
                    })
                    .collect()
            }

            // Handle entity UUID directories - show attribute files
            ino if cache.get_path(ino).map_or(false, |p| p.starts_with("/by-uuid/bridges/")) => {
                if let Some(path) = cache.get_path(ino) {
                    let path_clone = path.to_string();
                    if let Some(uuid_part) = path.strip_prefix("/by-uuid/bridges/") {
                        if let Some(uuid) = uuid_part.split('/').next() {
                            if let Some(entity) = cache.get_bridge_by_uuid(uuid) {
                                // Collect attribute names first
                                let attr_names: Vec<String> = entity.attributes.keys().cloned().collect();
                                drop(cache);

                                // Now allocate inodes
                                let mut result = Vec::new();
                                for attr_name in attr_names {
                                    let attr_path = format!("{}/{}", path_clone, attr_name);
                                    let inode = self.cache.write().unwrap().allocate_inode(&attr_path);
                                    result.push((inode, FileType::RegularFile, attr_name));
                                }
                                result
                            } else {
                                vec![]
                            }
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }

            ino if cache.get_path(ino).map_or(false, |p| p.starts_with("/by-uuid/ports/")) => {
                if let Some(path) = cache.get_path(ino) {
                    let path_clone = path.to_string();
                    if let Some(uuid_part) = path.strip_prefix("/by-uuid/ports/") {
                        if let Some(uuid) = uuid_part.split('/').next() {
                            if let Some(entity) = cache.get_port_by_uuid(uuid) {
                                // Collect attribute names first
                                let attr_names: Vec<String> = entity.attributes.keys().cloned().collect();
                                drop(cache);

                                // Now allocate inodes
                                let mut result = Vec::new();
                                for attr_name in attr_names {
                                    let attr_path = format!("{}/{}", path_clone, attr_name);
                                    let inode = self.cache.write().unwrap().allocate_inode(&attr_path);
                                    result.push((inode, FileType::RegularFile, attr_name));
                                }
                                result
                            } else {
                                vec![]
                            }
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }

            ino if cache.get_path(ino).map_or(false, |p| p.starts_with("/by-uuid/interfaces/")) => {
                if let Some(path) = cache.get_path(ino) {
                    let path_clone = path.to_string();
                    if let Some(uuid_part) = path.strip_prefix("/by-uuid/interfaces/") {
                        if let Some(uuid) = uuid_part.split('/').next() {
                            if let Some(entity) = cache.get_interface_by_uuid(uuid) {
                                // Collect attribute names first
                                let attr_names: Vec<String> = entity.attributes.keys().cloned().collect();
                                drop(cache);

                                // Now allocate inodes
                                let mut result = Vec::new();
                                for attr_name in attr_names {
                                    let attr_path = format!("{}/{}", path_clone, attr_name);
                                    let inode = self.cache.write().unwrap().allocate_inode(&attr_path);
                                    result.push((inode, FileType::RegularFile, attr_name));
                                }
                                result
                            } else {
                                vec![]
                            }
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
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
            } else if path.starts_with("/by-name/ports/") {
                let name = path.strip_prefix("/by-name/ports/").unwrap();
                if let Some(entity) = cache.get_port_by_name(name) {
                    let target = format!("../../by-uuid/ports/{}", entity.uuid);
                    reply.data(target.as_bytes());
                    return;
                }
            } else if path.starts_with("/by-name/interfaces/") {
                let name = path.strip_prefix("/by-name/interfaces/").unwrap();
                if let Some(entity) = cache.get_interface_by_name(name) {
                    let target = format!("../../by-uuid/interfaces/{}", entity.uuid);
                    reply.data(target.as_bytes());
                    return;
                }
            }
        }

        reply.error(libc::ENOENT);
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, _size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        let cache = self.cache.read().unwrap();

        if let Some(path) = cache.get_path(ino) {
            // Handle attribute file reads
            if path.starts_with("/by-uuid/bridges/") {
                if let Some(attr_part) = path.split('/').nth(4) { // /by-uuid/bridges/{uuid}/{attr}
                    if let Some(uuid_part) = path.split('/').nth(3) {
                        if let Some(entity) = cache.get_bridge_by_uuid(uuid_part) {
                            if let Some(value) = entity.attributes.get(attr_part) {
                                let json_str = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
                                let data = json_str.as_bytes();
                                let start = offset as usize;
                                if start < data.len() {
                                    reply.data(&data[start..]);
                                } else {
                                    reply.data(&[]);
                                }
                                return;
                            }
                        }
                    }
                }
            } else if path.starts_with("/by-uuid/ports/") {
                if let Some(attr_part) = path.split('/').nth(4) { // /by-uuid/ports/{uuid}/{attr}
                    if let Some(uuid_part) = path.split('/').nth(3) {
                        if let Some(entity) = cache.get_port_by_uuid(uuid_part) {
                            if let Some(value) = entity.attributes.get(attr_part) {
                                let json_str = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
                                let data = json_str.as_bytes();
                                let start = offset as usize;
                                if start < data.len() {
                                    reply.data(&data[start..]);
                                } else {
                                    reply.data(&[]);
                                }
                                return;
                            }
                        }
                    }
                }
            } else if path.starts_with("/by-uuid/interfaces/") {
                if let Some(attr_part) = path.split('/').nth(4) { // /by-uuid/interfaces/{uuid}/{attr}
                    if let Some(uuid_part) = path.split('/').nth(3) {
                        if let Some(entity) = cache.get_interface_by_uuid(uuid_part) {
                            if let Some(value) = entity.attributes.get(attr_part) {
                                let json_str = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
                                let data = json_str.as_bytes();
                                let start = offset as usize;
                                if start < data.len() {
                                    reply.data(&data[start..]);
                                } else {
                                    reply.data(&[]);
                                }
                                return;
                            }
                        }
                    }
                }
            }
        }

        reply.error(libc::ENOENT);
    }
}
