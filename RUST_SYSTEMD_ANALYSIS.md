# rust-systemd Integration Analysis

**Project:** OVS Port Agent (nm-monitor)  
**Date:** 2025-10-13  
**Analysis:** Would `/git/rust-systemd` benefit this project?

---

## Executive Summary

**Verdict:** ⚠️ **MIXED - Limited Benefit**

**Recommendation:** Adopt for daemon integration features (socket activation, notify, watchdog), but **NOT** for networkd operations.

---

## 1. CURRENT SYSTEMD USAGE IN PROJECT

### 1.1 Existing Dependencies

```toml
[dependencies]
systemd-journal-logger = "2"  # Journal logging only
zbus = { path = "/git/zbus/zbus", features = ["tokio"] }  # D-Bus communication
```

### 1.2 Current Systemd Integration Points

| Component | Current Approach | Files Affected |
|-----------|------------------|----------------|
| **Journal Logging** | `systemd-journal-logger` | `src/logging.rs` |
| **NetworkD D-Bus** | Manual zbus calls | `src/systemd_dbus.rs` |
| **NetworkD Commands** | Shell exec (`networkctl`) | `src/command.rs`, `src/services/` |
| **Service Lifecycle** | None | - |
| **Socket Activation** | None | - |
| **Watchdog** | None | - |

### 1.3 Code Examples

**Current Journal Logging:**
```rust
// src/logging.rs
use systemd_journal_logger::JournalLog;

pub fn init() -> Result<()> {
    if JournalLog::new()?.install().is_ok() {
        return Ok(());
    }
    // Fallback to env_logger
    env_logger::init();
    Ok(())
}
```

**Current NetworkD D-Bus:**
```rust
// src/systemd_dbus.rs
use zbus::{Connection, Proxy};

pub async fn get_network_state() -> Result<SystemdNetworkState> {
    let conn = Connection::system().await?;
    let manager = Proxy::new(
        &conn,
        "org.freedesktop.network1",
        "/org/freedesktop/network1",
        "org.freedesktop.network1.Manager",
    ).await?;
    
    let interfaces: Vec<...> = manager.call("ListLinks", &()).await?;
    // ...
}
```

---

## 2. WHAT RUST-SYSTEMD OFFERS

### 2.1 Available APIs

From `/git/rust-systemd`:

```rust
// Features available
pub mod journal;    // ✅ Journal read/write
pub mod daemon;     // ✅ Socket activation, notify, watchdog
pub mod bus;        // ❓ D-Bus interface (low-level)
pub mod login;      // ❌ Not relevant (seat/session management)
pub mod id128;      // ❌ Not relevant (128-bit IDs)
pub mod unit;       // ❓ Unit utilities (limited)
```

### 2.2 Detailed Capabilities

#### ✅ **journal** Module (200+ lines)

**Provides:**
- `journal::print()` - Write to journal
- `journal::send()` - Send structured journal entries
- `JournalLog` - log facade integration
- `Journal` - Read journal entries
- Structured field support

**Example:**
```rust
use systemd::journal;

// Write to journal
journal::print(3, "Application started");

// Structured logging
journal::send(&[
    "MESSAGE=Container interface created",
    "PRIORITY=6",
    "CONTAINER_ID=12345",
    "VMID=100",
    "BRIDGE=ovsbr0",
]);

// Read journal
let mut journal = Journal::open()?;
journal.seek(JournalSeek::Head)?;
while let Some(entry) = journal.next_entry()? {
    println!("{:?}", entry);
}
```

#### ✅ **daemon** Module (300+ lines)

**Provides:**
- `daemon::notify()` - Notify systemd of state changes
- `daemon::watchdog_enabled()` - Check if watchdog enabled
- `daemon::notify_watchdog()` - Reset watchdog timer
- `daemon::booted()` - Check if system booted with systemd
- `daemon::listen_fds()` - Socket activation support

**Example:**
```rust
use systemd::daemon;

// Notify systemd that daemon is ready
daemon::notify(false, &[("READY", "1")])?;

// Watchdog support
if let Ok(Some(duration)) = daemon::watchdog_enabled(false) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(duration / 2);
            daemon::notify_watchdog().ok();
        }
    });
}

// Socket activation
if let Ok(fds) = daemon::listen_fds(true) {
    for fd in fds {
        // Use pre-opened socket
        let listener = TcpListener::from_raw_fd(fd);
        // ...
    }
}
```

#### ❓ **bus** Module (Low-Level D-Bus)

**Provides:**
- Raw libsystemd sd-bus API
- Message construction/parsing
- Property get/set
- Method calls

**Assessment:** **NOT RECOMMENDED** - zbus is superior
- zbus is async/await native
- zbus has better Rust ergonomics
- zbus 5.x is modern and actively maintained
- zbus has procedural macros for interfaces

#### ❌ **login** Module (Not Relevant)

Provides seat/session management - not needed for network daemon.

#### ❌ **id128** Module (Not Relevant)

Provides 128-bit ID utilities - not needed.

---

## 3. WHAT RUST-SYSTEMD DOES NOT OFFER

### 3.1 Missing Functionality

**NO systemd-networkd APIs:**
- ❌ No `org.freedesktop.network1` interface helpers
- ❌ No high-level network management
- ❌ No networkctl wrapper
- ❌ Still need zbus for networkd D-Bus calls
- ❌ Still need shell commands for networkctl

**Conclusion:** rust-systemd has **ZERO** support for systemd-networkd operations.

### 3.2 What You'd Still Need

```rust
// Would still need this even with rust-systemd
use zbus::{Connection, Proxy};

pub async fn get_network_state() -> Result<...> {
    let conn = Connection::system().await?;
    let manager = Proxy::new(
        &conn,
        "org.freedesktop.network1",  // ← rust-systemd doesn't help here
        "/org/freedesktop/network1",
        "org.freedesktop.network1.Manager",
    ).await?;
    // ...
}
```

---

## 4. BENEFIT ANALYSIS

### 4.1 Potential Benefits

#### ✅ **Enhanced Journal Logging** (Medium Value)

**Current:**
```rust
// Simple logging via systemd-journal-logger
log::info!("Container interface created");
```

**With rust-systemd:**
```rust
// Structured logging with metadata
systemd::journal::send(&[
    "MESSAGE=Container interface created",
    "PRIORITY=6",
    "CONTAINER_ID=12345",
    "VMID=100",
    "BRIDGE=ovsbr0",
    "INTERFACE=vi100",
    "BLOCKCHAIN_HASH=abc123...",
]);

// Enables powerful queries:
// journalctl VMID=100
// journalctl BRIDGE=ovsbr0
// journalctl INTERFACE=vi100
```

**Value:**
- Better debugging with structured fields
- Easier log filtering and searching
- Better integration with systemd tooling
- **Effort:** Low (2-3 hours)

#### ✅ **Daemon Lifecycle Integration** (High Value)

**Current:** No systemd integration

**With rust-systemd:**
```rust
// systemd/ovs-port-agent.service
[Service]
Type=notify
WatchdogSec=30s

// src/main.rs
#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    // Initialize services
    let state = setup_state()?;
    
    // Notify systemd we're ready
    systemd::daemon::notify(false, &[("READY", "1")])?;
    
    // Start watchdog thread
    if let Ok(Some(duration)) = systemd::daemon::watchdog_enabled(false) {
        start_watchdog_thread(duration);
    }
    
    // Run service
    rpc::serve_with_state(state).await?;
    
    // Notify systemd we're stopping
    systemd::daemon::notify(false, &[("STOPPING", "1")])?;
    Ok(())
}

fn start_watchdog_thread(duration: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(duration / 2);
        loop {
            interval.tick().await;
            systemd::daemon::notify_watchdog().ok();
        }
    });
}
```

**Value:**
- Proper systemd service type (`Type=notify`)
- Watchdog protection against hangs
- Better service monitoring
- Clean shutdown handling
- **Effort:** Medium (4-6 hours)

#### ✅ **Socket Activation** (Low-Medium Value)

**Current:** Opens D-Bus connection directly

**With rust-systemd:**
```systemd
# systemd/ovs-port-agent.socket
[Socket]
ListenStream=/run/ovs-port-agent.sock
Accept=false

[Install]
WantedBy=sockets.target
```

```rust
// src/main.rs
if let Ok(fds) = systemd::daemon::listen_fds(true) {
    // Use pre-opened socket from systemd
    for fd in fds {
        // Connection already established by systemd
    }
} else {
    // Fallback: open connection normally
}
```

**Value:**
- Faster startup (systemd pre-opens sockets)
- Better resource management
- On-demand activation
- **Effort:** Medium (3-4 hours)

### 4.2 Limited/No Benefits

#### ❌ **NetworkD Operations** (No Benefit)

rust-systemd provides **ZERO** help for:
- systemd-networkd D-Bus calls
- networkctl command execution
- Network state queries
- Bridge management

**Conclusion:** Keep using zbus + command.rs for networkd.

#### ❌ **Bus Module** (No Benefit)

rust-systemd's `bus` module is low-level and inferior to zbus.

**Keep using:** zbus 5.x for all D-Bus operations.

---

## 5. COST-BENEFIT ANALYSIS

### 5.1 Benefits Summary

| Feature | Value | Effort | Priority |
|---------|-------|--------|----------|
| **Structured Journal Logging** | Medium | Low (2-3h) | ⭐⭐⭐ Medium |
| **Daemon Lifecycle (notify/watchdog)** | High | Medium (4-6h) | ⭐⭐⭐⭐ High |
| **Socket Activation** | Low-Med | Medium (3-4h) | ⭐⭐ Low |
| **Journal Reading** | Low | Low (1-2h) | ⭐ Very Low |
| **NetworkD Operations** | None | N/A | ❌ Not applicable |

### 5.2 Costs

**Dependency Addition:**
```toml
[dependencies]
systemd = { path = "/git/rust-systemd", features = ["journal", "daemon"] }
```

**Concerns:**
- ✅ License: LGPL 2.1+ with linking exception (compatible)
- ✅ Maintenance: Actively maintained
- ⚠️ Additional dependency: +1 crate (but well-established)
- ⚠️ Binds to libsystemd: Requires libsystemd.so at runtime (already required)

### 5.3 ROI Assessment

**High ROI:**
- ✅ Daemon lifecycle integration (notify/watchdog)
- ✅ Structured journal logging

**Low ROI:**
- ⚠️ Socket activation (nice-to-have)
- ❌ Journal reading (rarely needed)

**No ROI:**
- ❌ Bus module (zbus is better)
- ❌ NetworkD operations (not supported)

---

## 6. RECOMMENDATION

### 6.1 Recommended Adoption Strategy

**Phase 1: High-Value Features (Recommended)**

```toml
[dependencies]
systemd = { path = "/git/rust-systemd", features = ["journal", "daemon"], default-features = false }
```

**Implement:**
1. ✅ **Daemon notify/watchdog** (Priority: High)
   - Add `Type=notify` to systemd service
   - Implement watchdog thread
   - Proper startup/shutdown notifications

2. ✅ **Structured journal logging** (Priority: Medium)
   - Replace simple log calls with structured fields
   - Add metadata (VMID, bridge, interface, etc.)
   - Enable powerful filtering

**Do NOT implement:**
- ❌ Bus module (keep zbus)
- ❌ Socket activation (low priority)
- ❌ Journal reading (not needed)

### 6.2 Code Changes Required

**1. Update Cargo.toml:**
```toml
[dependencies]
systemd = { path = "/git/rust-systemd", features = ["journal", "daemon"], default-features = false }
systemd-journal-logger = "2"  # Can keep as fallback
zbus = { path = "/git/zbus/zbus", features = ["tokio"] }  # KEEP for networkd
```

**2. Update main.rs:**
```rust
#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    // Check if running under systemd
    let systemd_managed = systemd::daemon::booted()
        .unwrap_or(false);
    
    let cfg = config::Config::load(args.config.as_deref())?;
    
    // ... initialize services ...
    
    if systemd_managed {
        // Notify systemd we're ready
        systemd::daemon::notify(false, &[
            ("READY", "1"),
            ("STATUS", "OVS Port Agent initialized"),
        ])?;
        
        // Start watchdog if enabled
        if let Ok(Some(duration)) = systemd::daemon::watchdog_enabled(false) {
            start_watchdog_thread(duration);
        }
    }
    
    info!("OVS Port Agent initialized successfully");
    rpc::serve_with_state(rpc_state).await?;
    
    if systemd_managed {
        systemd::daemon::notify(false, &[("STOPPING", "1")])?;
    }
    
    Ok(())
}
```

**3. Enhanced Ledger Logging:**
```rust
// src/ledger.rs
pub fn add_block(&mut self, block: Block) -> Result<String> {
    // ... existing code ...
    
    // Structured journal logging
    if let Ok(systemd) = systemd::daemon::booted() {
        if systemd {
            systemd::journal::send(&[
                &format!("MESSAGE=Added block {} to blockchain", block.height),
                "PRIORITY=6",
                &format!("BLOCK_HEIGHT={}", block.height),
                &format!("CATEGORY={}", block.category),
                &format!("ACTION={}", block.action),
                &format!("BLOCK_HASH={}", hash),
            ]).ok();
        }
    }
    
    Ok(hash)
}
```

**4. Update systemd service file:**
```ini
[Unit]
Description=OVS Port Agent
After=network.target

[Service]
Type=notify          # ← Changed from simple
ExecStart=/usr/local/bin/ovs-port-agent run
Restart=on-failure
RestartSec=5s
WatchdogSec=30s      # ← Added watchdog

# Resource limits
MemoryMax=128M
CPUQuota=50%

[Install]
WantedBy=multi-user.target
```

### 6.3 Migration Effort

| Task | Effort | Priority |
|------|--------|----------|
| Add dependency | 5 min | - |
| Implement notify/watchdog | 4-6 hours | High |
| Update systemd service file | 30 min | High |
| Add structured logging | 2-3 hours | Medium |
| Testing | 2-3 hours | High |
| **TOTAL** | **9-13 hours** | - |

---

## 7. ALTERNATIVE: KEEP CURRENT APPROACH

### 7.1 Current Stack Works Well

**Current approach is sufficient for:**
- ✅ Basic journal logging (systemd-journal-logger)
- ✅ NetworkD D-Bus calls (zbus)
- ✅ Network operations (command.rs)
- ✅ Service lifecycle (systemd Type=simple)

**Reasons to keep current:**
- Working reliably in production
- No critical gaps in functionality
- Simpler dependency tree
- Less maintenance burden

### 7.2 Manual Alternatives

**For notify/watchdog:**
Can implement manually without rust-systemd:
```rust
// Manual systemd notify via socket
use std::os::unix::net::UnixDatagram;

fn notify_systemd(state: &str) -> Result<()> {
    if let Ok(socket_path) = std::env::var("NOTIFY_SOCKET") {
        let socket = UnixDatagram::unbound()?;
        socket.send_to(state.as_bytes(), &socket_path)?;
    }
    Ok(())
}

notify_systemd("READY=1\n")?;
```

**Trade-off:** More code, less type-safe, but no dependency.

---

## 8. FINAL VERDICT

### 8.1 Adoption Recommendation

**✅ YES - Adopt rust-systemd, but LIMITED scope:**

**Use for:**
- ✅ Daemon lifecycle (notify/watchdog)
- ✅ Structured journal logging

**Do NOT use for:**
- ❌ D-Bus operations (keep zbus)
- ❌ NetworkD operations (keep command.rs + zbus)
- ❌ Socket activation (low priority)

**Dependency:**
```toml
systemd = { path = "/git/rust-systemd", features = ["journal", "daemon"], default-features = false }
```

### 8.2 Priority Assessment

| Scenario | Recommendation |
|----------|----------------|
| **Production system with systemd** | ✅ **Adopt** (High value for proper integration) |
| **Development/testing** | ⚠️ **Optional** (Nice-to-have, not critical) |
| **Non-systemd systems** | ❌ **Skip** (No value) |
| **Time-constrained** | ⚠️ **Defer** (Can add later, not urgent) |

### 8.3 Expected Improvements

**With rust-systemd adoption:**
- ✅ Better systemd integration (proper service type, watchdog)
- ✅ Enhanced debugging (structured logs with metadata)
- ✅ Improved reliability (watchdog catches hangs)
- ✅ Better monitoring (systemd knows service state)
- ❌ **NO** improvement to networkd operations

---

## 9. CONCLUSION

**rust-systemd** would provide **moderate benefit** to this project:

**✅ Recommended Uses:**
1. **Daemon lifecycle integration** - High value for production deployments
2. **Structured journal logging** - Medium value for debugging

**❌ NOT Recommended:**
- NetworkD operations - rust-systemd provides zero value here
- D-Bus interface - zbus is superior
- Socket activation - Low priority for this use case

**Overall Assessment:** 
- **Value:** ⭐⭐⭐ Medium (3/5)
- **Effort:** 9-13 hours
- **Priority:** Medium (implement if time permits)

**Bottom Line:** rust-systemd is a good addition for proper systemd integration, but does NOT help with your core networkd/OVS operations. The main benefit is better daemon lifecycle management and debugging capabilities.

---

## 10. IMPLEMENTATION CHECKLIST

If you decide to adopt rust-systemd:

- [ ] Add dependency to Cargo.toml
- [ ] Implement systemd notify on startup
- [ ] Implement watchdog thread
- [ ] Update systemd service file (Type=notify, WatchdogSec)
- [ ] Add structured journal logging to key operations
- [ ] Test watchdog behavior (kill -STOP to simulate hang)
- [ ] Test notify behavior (systemctl status shows correct state)
- [ ] Update documentation
- [ ] Verify backward compatibility (runs without systemd)

**Estimated Total Effort:** 9-13 hours
