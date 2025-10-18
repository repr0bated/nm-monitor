# Universal Element Blockchain System

## 🌍 Vision

**Every element in the system has its own blockchain** tracking all modifications from creation to deletion. Not just network - EVERYTHING.

## 🎯 System Elements

### Network Layer
- Interfaces
- Bridges  
- Routes
- Firewall rules
- DNS entries

### Filesystem Layer
- Files
- Directories
- Symlinks
- Mount points
- Block devices

### Process Layer
- Running processes
- Services
- Systemd units
- Cron jobs
- Timers

### Configuration Layer
- Config files (`/etc/`)
- Environment variables
- Kernel parameters (sysctl)
- Module configs

### User/Permission Layer
- Users
- Groups
- SSH keys
- Sudo rules
- SELinux/AppArmor policies

### Package Layer
- Installed packages
- Dependencies
- Repositories
- Package configs

### System State
- Boot parameters
- Kernel modules
- System time
- Hostname
- Locale

## 🏗️ Universal Element Blockchain Architecture

```rust
/// Universal element type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ElementType {
    // Network
    NetworkInterface,
    NetworkBridge,
    NetworkRoute,
    FirewallRule,
    
    // Filesystem
    File,
    Directory,
    Symlink,
    MountPoint,
    BlockDevice,
    
    // Process
    Process,
    Service,
    SystemdUnit,
    CronJob,
    Timer,
    
    // Configuration
    ConfigFile,
    EnvVar,
    SysctlParam,
    KernelModule,
    
    // User/Permission
    User,
    Group,
    SshKey,
    SudoRule,
    SeLinuxPolicy,
    
    // Package
    Package,
    Repository,
    
    // System
    BootParam,
    Hostname,
    SystemTime,
}

/// Universal element blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalElement {
    /// Unique identifier (path, name, PID, etc.)
    pub id: String,
    
    /// Element type
    pub element_type: ElementType,
    
    /// Element blockchain (all modifications)
    pub blockchain: ElementBlockchain,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
    
    /// Current hash (quick access)
    pub current_hash: String,
    
    /// Parent element (for hierarchy)
    pub parent_id: Option<String>,
    
    /// Child elements
    pub children: Vec<String>,
}

impl UniversalElement {
    /// Create new element with genesis block
    pub fn new(
        id: String,
        element_type: ElementType,
        initial_state: serde_json::Value,
        actor: String,
    ) -> Result<Self> {
        let blockchain = ElementBlockchain::new(
            id.clone(),
            format!("{:?}", element_type),
            initial_state,
            actor,
        )?;
        
        let current_hash = blockchain.current_hash.clone();
        
        Ok(Self {
            id,
            element_type,
            blockchain,
            metadata: HashMap::new(),
            current_hash,
            parent_id: None,
            children: Vec::new(),
        })
    }
    
    /// Track modification
    pub fn modify(
        &mut self,
        new_state: serde_json::Value,
        modification_type: String,
        actor: String,
        reason: Option<String>,
    ) -> Result<String> {
        let hash = self.blockchain.add_modification(
            new_state,
            modification_type,
            actor,
            reason,
        )?;
        
        self.current_hash = hash.clone();
        Ok(hash)
    }
}
```

## 📂 Storage Structure

```
/var/lib/universal-blockchain/
├── network/
│   ├── interfaces/
│   │   ├── eth0.jsonl
│   │   ├── wlan0.jsonl
│   │   └── ovsbr0.jsonl
│   ├── routes/
│   │   └── default-route.jsonl
│   └── firewall/
│       └── rule-001.jsonl
│
├── filesystem/
│   ├── files/
│   │   ├── etc_hostname.jsonl          # /etc/hostname
│   │   ├── etc_passwd.jsonl            # /etc/passwd
│   │   └── etc_ssh_sshd_config.jsonl   # /etc/ssh/sshd_config
│   ├── directories/
│   │   └── var_www.jsonl
│   └── mounts/
│       └── mnt_data.jsonl
│
├── processes/
│   ├── services/
│   │   ├── nginx.jsonl
│   │   ├── postgresql.jsonl
│   │   └── ovs-port-agent.jsonl
│   └── systemd-units/
│       └── nginx_service.jsonl
│
├── config/
│   ├── sysctl/
│   │   └── net_ipv4_ip_forward.jsonl
│   └── kernel-modules/
│       └── openvswitch.jsonl
│
├── users/
│   ├── root.jsonl
│   ├── alice.jsonl
│   └── bob.jsonl
│
├── packages/
│   ├── nginx.jsonl
│   ├── postgresql.jsonl
│   └── python3.jsonl
│
└── system/
    ├── hostname.jsonl
    └── boot-params.jsonl
```

## 🔍 Universal Element Manager

```rust
use std::path::PathBuf;

pub struct UniversalElementManager {
    base_path: PathBuf,
    cache: HashMap<String, UniversalElement>,
}

impl UniversalElementManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        fs::create_dir_all(&base_path)?;
        
        Ok(Self {
            base_path,
            cache: HashMap::new(),
        })
    }
    
    /// Track any system element
    pub fn track_element(
        &mut self,
        id: String,
        element_type: ElementType,
        state: serde_json::Value,
        actor: String,
    ) -> Result<String> {
        let element = UniversalElement::new(id.clone(), element_type, state, actor)?;
        let hash = element.current_hash.clone();
        
        self.save_element(&element)?;
        self.cache.insert(id, element);
        
        Ok(hash)
    }
    
    /// Track file modification
    pub fn track_file(&mut self, path: &str, actor: String) -> Result<String> {
        let id = Self::path_to_id(path);
        let content = fs::read_to_string(path)?;
        let metadata = fs::metadata(path)?;
        
        let state = json!({
            "path": path,
            "content_hash": Self::hash_content(&content),
            "size": metadata.len(),
            "permissions": format!("{:o}", metadata.permissions().mode()),
            "modified": metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs(),
        });
        
        if let Some(element) = self.cache.get_mut(&id) {
            // Existing file - track modification
            element.modify(state, "modified".to_string(), actor, None)
        } else {
            // New file - track creation
            self.track_element(id, ElementType::File, state, actor)
        }
    }
    
    /// Track service state change
    pub fn track_service(&mut self, name: &str, new_state: &str, actor: String) -> Result<String> {
        let id = format!("service:{}", name);
        
        // Get current service status
        let output = Command::new("systemctl")
            .args(["status", name, "--no-pager"])
            .output()?;
        
        let status = String::from_utf8_lossy(&output.stdout);
        
        let state = json!({
            "service": name,
            "state": new_state,
            "status_output": status,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Some(element) = self.cache.get_mut(&id) {
            element.modify(state, "state_change".to_string(), actor, Some(format!("Service {} {}", name, new_state)))
        } else {
            self.track_element(id, ElementType::Service, state, actor)
        }
    }
    
    /// Track package installation/removal
    pub fn track_package(&mut self, name: &str, version: &str, action: &str, actor: String) -> Result<String> {
        let id = format!("package:{}", name);
        
        let state = json!({
            "package": name,
            "version": version,
            "action": action,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Some(element) = self.cache.get_mut(&id) {
            element.modify(state, action.to_string(), actor, Some(format!("Package {} {}", name, action)))
        } else {
            self.track_element(id, ElementType::Package, state, actor)
        }
    }
    
    /// Track user creation/modification
    pub fn track_user(&mut self, username: &str, action: &str, actor: String) -> Result<String> {
        let id = format!("user:{}", username);
        
        // Parse /etc/passwd
        let passwd_entry = fs::read_to_string("/etc/passwd")?
            .lines()
            .find(|l| l.starts_with(&format!("{}:", username)))
            .ok_or_else(|| anyhow::anyhow!("User not found"))?
            .to_string();
        
        let parts: Vec<&str> = passwd_entry.split(':').collect();
        
        let state = json!({
            "username": username,
            "uid": parts.get(2).unwrap_or(&""),
            "gid": parts.get(3).unwrap_or(&""),
            "home": parts.get(5).unwrap_or(&""),
            "shell": parts.get(6).unwrap_or(&""),
        });
        
        if let Some(element) = self.cache.get_mut(&id) {
            element.modify(state, action.to_string(), actor, Some(format!("User {} {}", username, action)))
        } else {
            self.track_element(id, ElementType::User, state, actor)
        }
    }
    
    /// Track network interface
    pub fn track_interface(&mut self, name: &str, actor: String) -> Result<String> {
        let id = format!("interface:{}", name);
        
        // Get interface state
        let output = Command::new("ip")
            .args(["addr", "show", name])
            .output()?;
        
        let ip_info = String::from_utf8_lossy(&output.stdout);
        
        let state = json!({
            "interface": name,
            "ip_info": ip_info,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        if let Some(element) = self.cache.get_mut(&id) {
            element.modify(state, "state_update".to_string(), actor, None)
        } else {
            self.track_element(id, ElementType::NetworkInterface, state, actor)
        }
    }
    
    fn path_to_id(path: &str) -> String {
        path.replace('/', "_").trim_start_matches('_').to_string()
    }
    
    fn hash_content(content: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    fn save_element(&self, element: &UniversalElement) -> Result<()> {
        let type_dir = self.base_path.join(format!("{:?}", element.element_type).to_lowercase());
        fs::create_dir_all(&type_dir)?;
        
        let element_file = type_dir.join(format!("{}.jsonl", element.id));
        
        // Append-only: write all blocks
        let mut content = String::new();
        for block in &element.blockchain.blocks {
            content.push_str(&serde_json::to_string(block)?);
            content.push('\n');
        }
        
        fs::write(&element_file, content)?;
        Ok(())
    }
    
    /// Query system state at any point in time
    pub fn query_state_at(
        &self,
        element_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<serde_json::Value>> {
        if let Some(element) = self.cache.get(element_id) {
            // Find block closest to timestamp
            for block in element.blockchain.blocks.iter().rev() {
                let block_time = DateTime::parse_from_rfc3339(&block.timestamp)?;
                if block_time.timestamp() <= timestamp.timestamp() {
                    return Ok(Some(block.state_snapshot.clone()));
                }
            }
        }
        Ok(None)
    }
    
    /// Get all elements of a type
    pub fn list_by_type(&self, element_type: ElementType) -> Vec<&UniversalElement> {
        self.cache
            .values()
            .filter(|e| e.element_type == element_type)
            .collect()
    }
    
    /// Verify entire system integrity
    pub fn verify_all(&self) -> Result<HashMap<String, bool>> {
        let mut results = HashMap::new();
        
        for (id, element) in &self.cache {
            results.insert(id.clone(), element.blockchain.verify_chain()?);
        }
        
        Ok(results)
    }
}
```

## 🎣 System Hooks

### File System Watcher (inotify)

```rust
use notify::{Watcher, RecursiveMode, Event};

pub struct FileSystemHook {
    manager: Arc<Mutex<UniversalElementManager>>,
    watcher: RecommendedWatcher,
}

impl FileSystemHook {
    pub fn new(manager: Arc<Mutex<UniversalElementManager>>) -> Result<Self> {
        let mgr = manager.clone();
        
        let watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                let mut manager = mgr.lock().unwrap();
                
                match event.kind {
                    EventKind::Modify(_) => {
                        for path in event.paths {
                            let _ = manager.track_file(
                                path.to_str().unwrap(),
                                "filesystem".to_string(),
                            );
                        }
                    }
                    _ => {}
                }
            }
        })?;
        
        Ok(Self { manager, watcher })
    }
    
    pub fn watch_path(&mut self, path: &str) -> Result<()> {
        self.watcher.watch(Path::new(path), RecursiveMode::Recursive)?;
        Ok(())
    }
}

// Usage
let hook = FileSystemHook::new(manager.clone())?;
hook.watch_path("/etc")?;  // Track all /etc changes
hook.watch_path("/var/www")?;  // Track web content
```

### systemd Hook

```rust
pub struct SystemdHook {
    manager: Arc<Mutex<UniversalElementManager>>,
}

impl SystemdHook {
    /// Monitor systemd unit changes via D-Bus
    pub async fn monitor(&self) -> Result<()> {
        let connection = zbus::Connection::system().await?;
        
        let proxy = systemd1::ManagerProxy::new(&connection).await?;
        
        let mut changes = proxy.receive_unit_new().await?;
        
        while let Some(signal) = changes.next().await {
            let args = signal.args()?;
            let unit_name = args.name;
            let unit_path = args.path;
            
            let mut manager = self.manager.lock().unwrap();
            manager.track_service(&unit_name, "started", "systemd".to_string())?;
        }
        
        Ok(())
    }
}
```

### Package Manager Hook

```rust
pub struct PackageHook;

impl PackageHook {
    /// Hook into dpkg/apt (Debian/Ubuntu)
    pub fn hook_dpkg(manager: &mut UniversalElementManager) -> Result<()> {
        // Parse /var/log/dpkg.log
        let log = fs::read_to_string("/var/log/dpkg.log")?;
        
        for line in log.lines().rev().take(100) {
            if line.contains("install") || line.contains("remove") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let action = parts[2];
                    let package = parts[3].split(':').next().unwrap();
                    
                    manager.track_package(package, "unknown", action, "dpkg".to_string())?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Hook into DNF/YUM (RHEL/Fedora)
    pub fn hook_dnf(manager: &mut UniversalElementManager) -> Result<()> {
        // Parse /var/log/dnf.log
        // Similar implementation
        Ok(())
    }
}
```

## 🖥️ CLI Interface

```bash
# Universal element tracking

# Track a file
ueb track file /etc/nginx/nginx.conf

# Track a service
ueb track service nginx

# Track a package
ueb track package postgresql

# Track a user
ueb track user alice

# Query system state at timestamp
ueb query interface:eth0 --at "2025-10-13T10:00:00Z"

# View element history
ueb log service:nginx

# Verify integrity
ueb verify --all

# Find all elements of type
ueb list --type=File

# System-wide diff
ueb diff --from "2025-10-13T00:00:00Z" --to "2025-10-13T23:59:59Z"

# Rollback element
ueb rollback service:nginx --to-height 3

# Export blockchain
ueb export interface:eth0 --format json

# Verify against known-good state
ueb audit --baseline /var/lib/baseline.json
```

## 🔄 Integration Examples

### 1. Track Config File Changes

```rust
// Watch /etc for changes
let mut manager = UniversalElementManager::new("/var/lib/universal-blockchain")?;

manager.track_file("/etc/nginx/nginx.conf", "admin".to_string())?;
manager.track_file("/etc/ssh/sshd_config", "admin".to_string())?;
manager.track_file("/etc/hostname", "system".to_string())?;
```

### 2. Track Service Lifecycle

```rust
// Service started
manager.track_service("nginx", "started", "systemd".to_string())?;

// Service reloaded
manager.track_service("nginx", "reloaded", "admin".to_string())?;

// Service stopped
manager.track_service("nginx", "stopped", "systemd".to_string())?;
```

### 3. Track Package Operations

```rust
// Package installed
manager.track_package("nginx", "1.20.1", "installed", "apt".to_string())?;

// Package upgraded
manager.track_package("nginx", "1.20.2", "upgraded", "apt".to_string())?;

// Package removed
manager.track_package("nginx", "1.20.2", "removed", "apt".to_string())?;
```

### 4. Time Travel Queries

```rust
// What was the nginx config on Oct 1st?
let state = manager.query_state_at(
    "file:etc_nginx_nginx_conf",
    Utc.ymd(2025, 10, 1).and_hms(0, 0, 0),
)?;

println!("Nginx config on Oct 1st: {:?}", state);
```

## 🎯 Use Cases

### Security Auditing
```bash
# What changed in /etc in the last 24 hours?
ueb changes --path /etc --since "24 hours ago"

# Who modified this config?
ueb log file:/etc/ssh/sshd_config

# Detect unauthorized changes
ueb verify --baseline /var/lib/baseline.json
```

### Compliance
```bash
# Generate compliance report
ueb report --standard=pci-dss --from "2025-01-01" --to "2025-12-31"

# Prove config was correct at audit time
ueb state file:/etc/pam.d/common-auth --at "2025-03-15T14:30:00Z"
```

### Incident Response
```bash
# What changed before the incident?
ueb diff --from "2025-10-13T09:00:00Z" --to "2025-10-13T09:05:00Z"

# Rollback to known-good state
ueb rollback-all --to "2025-10-13T08:00:00Z"
```

### Configuration Management
```bash
# Track all system state
ueb snapshot --name "pre-upgrade"

# Apply changes
apt upgrade -y

# Verify changes
ueb diff --from-snapshot "pre-upgrade"

# Rollback if needed
ueb restore --snapshot "pre-upgrade"
```

## 🚀 Advanced Features

### Content-Addressable Storage

```rust
// Store by hash, deduplicate automatically
pub struct ContentAddressableStore {
    store_path: PathBuf,
}

impl ContentAddressableStore {
    pub fn store(&self, content: &[u8]) -> Result<String> {
        let hash = self.hash(content);
        let store_file = self.store_path.join(&hash);
        
        if !store_file.exists() {
            fs::write(&store_file, content)?;
        }
        
        Ok(hash)
    }
    
    pub fn retrieve(&self, hash: &str) -> Result<Vec<u8>> {
        let store_file = self.store_path.join(hash);
        Ok(fs::read(&store_file)?)
    }
}
```

### Distributed Sync

```rust
// Sync element blockchains across nodes
pub struct DistributedSync {
    local: UniversalElementManager,
    peers: Vec<String>,
}

impl DistributedSync {
    pub async fn sync_element(&self, element_id: &str) -> Result<()> {
        for peer in &self.peers {
            // Fetch peer's blockchain
            let peer_chain = self.fetch_from_peer(peer, element_id).await?;
            
            // Merge chains (longest chain wins)
            self.local.merge_chain(element_id, peer_chain)?;
        }
        Ok(())
    }
}
```

### Smart Contracts (Actions on Events)

```rust
pub struct SmartAction {
    trigger: Trigger,
    action: Box<dyn Fn(&ElementBlock) -> Result<()>>,
}

pub enum Trigger {
    OnModify { element_type: ElementType },
    OnCreate { element_type: ElementType },
    OnDelete { element_type: ElementType },
}

// Example: Auto-backup on config change
let backup_action = SmartAction {
    trigger: Trigger::OnModify { element_type: ElementType::ConfigFile },
    action: Box::new(|block| {
        // Backup the file
        fs::copy(
            &block.state_snapshot["path"],
            format!("/backup/{}", block.hash),
        )?;
        Ok(())
    }),
};
```

## 📊 Performance Considerations

### Optimization Strategies

1. **Lazy Loading**: Load chains on-demand
2. **Caching**: Keep hot chains in memory
3. **Indexing**: Build indices for fast lookup
4. **Compaction**: Merge old blocks periodically
5. **Sharding**: Split by element type

### Storage Estimates

| Elements | Blocks/Element | Storage |
|----------|----------------|---------|
| 1,000 files | 10 blocks | ~2MB |
| 100 services | 50 blocks | ~1MB |
| 500 packages | 5 blocks | ~0.5MB |
| **Total** | | **~3.5MB** |

Very manageable!

## 🎉 Summary

This creates a **universal, tamper-evident, content-addressable system** where:

✅ **Every element** has its own blockchain  
✅ **Every modification** is recorded  
✅ **Complete audit trail** for everything  
✅ **Time travel** to any point  
✅ **Cryptographic proof** of integrity  
✅ **Git-like** workflow for the entire system  

**This is the future of system administration!** 🚀
