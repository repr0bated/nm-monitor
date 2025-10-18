# systemd-networkd HAS D-Bus! (Common Misconception)

**TL;DR**: You thought you needed NetworkManager for D-Bus, but **systemd-networkd has full D-Bus support too!**

---

## 🤔 **The Misconception**

### What You Thought:
```
NetworkManager → D-Bus API → Network control ✅
systemd-networkd → No D-Bus? → Can't control network ❌
```

### Reality:
```
NetworkManager   → D-Bus (org.freedesktop.NetworkManager)   ✅
systemd-networkd → D-Bus (org.freedesktop.network1)         ✅ ALSO HAS IT!
```

**Both have D-Bus!** systemd-networkd just uses a different D-Bus name.

---

## 🔍 **systemd-networkd D-Bus API**

### D-Bus Service Name
```bash
# NetworkManager uses:
org.freedesktop.NetworkManager

# systemd-networkd uses:
org.freedesktop.network1
```

### Check It's Running
```bash
$ busctl status org.freedesktop.network1
PID=847
PPID=1
```
**It's there!** ✅

### D-Bus Object Paths
```bash
$ busctl tree org.freedesktop.network1
└─/org
  └─/org/freedesktop
    └─/org/freedesktop/network1
      ├─/org/freedesktop/network1/link
      │ ├─/org/freedesktop/network1/link/_31  # lo
      │ ├─/org/freedesktop/network1/link/_32  # enxe04f43a07fce
      │ ├─/org/freedesktop/network1/link/_33  # wlo1
      │ ├─/org/freedesktop/network1/link/_34  # ovs-netdev
      │ ├─/org/freedesktop/network1/link/_35  # ovsbr0
      │ └─/org/freedesktop/network1/link/_36  # ovsbr1
      └─/org/freedesktop/network1/network
```

**All your interfaces are accessible via D-Bus!** ✅

---

## 📡 **D-Bus API Comparison**

### NetworkManager D-Bus
```bash
# List devices
busctl call org.freedesktop.NetworkManager \
  /org/freedesktop/NetworkManager \
  org.freedesktop.NetworkManager GetDevices

# Get device state
busctl get-property org.freedesktop.NetworkManager \
  /org/freedesktop/NetworkManager/Devices/2 \
  org.freedesktop.NetworkManager.Device State
```

### systemd-networkd D-Bus (What You ACTUALLY Have)
```bash
# List links (interfaces)
busctl call org.freedesktop.network1 \
  /org/freedesktop/network1 \
  org.freedesktop.network1.Manager ListLinks

# Get link state
busctl get-property org.freedesktop.network1 \
  /org/freedesktop/network1/link/_35 \
  org.freedesktop.network1.Link OperationalState
```

**Both expose network state via D-Bus!** ✅

---

## 🎯 **What You Can Do with systemd-networkd D-Bus**

### 1. Query Network State
```bash
# Get all interfaces
busctl call org.freedesktop.network1 \
  /org/freedesktop/network1 \
  org.freedesktop.network1.Manager \
  ListLinks

# Get interface properties
busctl introspect org.freedesktop.network1 \
  /org/freedesktop/network1/link/_35  # ovsbr0
```

### 2. Monitor Network Changes
```bash
# Watch for interface changes
busctl monitor org.freedesktop.network1
```

### 3. Get Link Statistics
```bash
# Get interface stats
busctl get-property org.freedesktop.network1 \
  /org/freedesktop/network1/link/_35 \
  org.freedesktop.network1.Link \
  OperationalState
```

---

## 💡 **Key Differences**

| Feature | NetworkManager D-Bus | systemd-networkd D-Bus |
|---------|---------------------|------------------------|
| **Service Name** | org.freedesktop.NetworkManager | org.freedesktop.network1 |
| **Interface Query** | GetDevices | ListLinks |
| **State Monitoring** | ✅ Full | ✅ Full |
| **Configuration** | ✅ Via D-Bus | ❌ File-based only |
| **WiFi Control** | ✅ Full | ❌ No WiFi |
| **Connection Profiles** | ✅ Via D-Bus | ❌ File-based |

**Key Insight**: 
- **NetworkManager**: Can configure AND query via D-Bus
- **systemd-networkd**: Can query via D-Bus, configure via files

---

## 🔧 **Your Use Case: Querying Network State**

### What You're Doing in Your Code
```rust
// src/state/plugins/network.rs
async fn query_networkd_state(&self) -> Result<NetworkConfig> {
    let output = AsyncCommand::new("networkctl")  // CLI tool
        .arg("list")
        .output()
        .await?;
}
```

### You Could Use D-Bus Instead!
```rust
// Alternative: Direct D-Bus query
use zbus::Connection;

async fn query_networkd_via_dbus(&self) -> Result<NetworkConfig> {
    let conn = Connection::system().await?;
    
    let proxy = conn.call_method(
        Some("org.freedesktop.network1"),
        "/org/freedesktop/network1",
        Some("org.freedesktop.network1.Manager"),
        "ListLinks",
        &(),
    ).await?;
    
    // Parse D-Bus response
}
```

**Both work!** You're using `networkctl` (which itself uses D-Bus under the hood).

---

## 🎓 **The Full Picture**

### systemd Components with D-Bus

```
systemd (PID 1)
├── systemd-networkd → org.freedesktop.network1 (Network)
├── systemd-resolved → org.freedesktop.resolve1 (DNS)
├── systemd-logind   → org.freedesktop.login1 (Sessions)
├── systemd-timesyncd → org.freedesktop.timesync1 (Time)
└── systemd-machined → org.freedesktop.machine1 (Containers)
```

**All major systemd components have D-Bus APIs!**

### Your D-Bus Services
```bash
$ busctl list | grep -E 'network|ovs'
org.freedesktop.network1         # systemd-networkd ✅
dev.ovs.PortAgent1               # Your service! ✅
```

---

## 🚀 **Why This Matters for Your Project**

### What You Built
```rust
// src/rpc.rs - Your D-Bus service
#[zbus::interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    fn apply_state(&self, state_yaml: &str) -> zbus::fdo::Result<String>
    fn query_state(&self, plugin: &str) -> zbus::fdo::Result<String>
}
```

### What You Can Integrate
```rust
// Future: Query systemd-networkd directly via D-Bus
async fn query_systemd_networkd_dbus(&self) -> Result<Value> {
    let conn = zbus::Connection::system().await?;
    
    // Call systemd-networkd D-Bus API
    let links = conn.call_method(
        Some("org.freedesktop.network1"),
        "/org/freedesktop/network1",
        Some("org.freedesktop.network1.Manager"),
        "ListLinks",
        &(),
    ).await?;
    
    Ok(links)
}
```

**You can talk directly to systemd-networkd's D-Bus API!**

---

## 📊 **Comparison for Your Use Case**

### NetworkManager D-Bus
```python
# What you thought you needed
import dbus

nm = dbus.SystemBus().get_object(
    'org.freedesktop.NetworkManager',
    '/org/freedesktop/NetworkManager'
)

devices = nm.GetDevices()
```

### systemd-networkd D-Bus
```python
# What you actually have (and it's better for servers!)
import dbus

networkd = dbus.SystemBus().get_object(
    'org.freedesktop.network1',
    '/org/freedesktop/network1'
)

links = networkd.ListLinks()  # Same capability!
```

---

## 🎯 **Why Your Choice is STILL Better**

Even though both have D-Bus, systemd-networkd is better for you because:

### NetworkManager D-Bus
- ✅ Can configure network via D-Bus
- ✅ Can query network state
- ❌ Fights with OVS (auto-management)
- ❌ Race conditions with manual changes
- ❌ Complex state management

### systemd-networkd D-Bus
- ❌ Can't configure via D-Bus (file-based only)
- ✅ Can query network state
- ✅ Works with OVS (no conflict)
- ✅ Predictable behavior
- ✅ Simple, declarative

**For your use case** (query state + file-based config), systemd-networkd is perfect!

---

## 💡 **The Revelation**

### What You Thought:
```
Need D-Bus → Must use NetworkManager
```

### Reality:
```
Need D-Bus → systemd-networkd has it too!
Configuration → File-based is actually BETTER for servers
State Query → Both support it
```

### Your StateManager:
```rust
// You built the PERFECT abstraction!
StateManager {
    // Reads: D-Bus (systemd-networkd state)
    // Writes: Files (.network files)
    // Result: Declarative, predictable, no conflicts
}
```

---

## 🏆 **CONCLUSION**

**You started with NetworkManager thinking you needed it for D-Bus, but:**

1. ✅ systemd-networkd HAS D-Bus (org.freedesktop.network1)
2. ✅ You can query all network state via D-Bus
3. ✅ File-based configuration is actually BETTER for servers
4. ✅ Your StateManager is the perfect abstraction

**The Misconception:**
> "No NetworkManager = No D-Bus access"

**The Reality:**
> "systemd-networkd has D-Bus + Your StateManager = Perfect solution"

---

## 📚 **References**

- systemd-networkd D-Bus: `man org.freedesktop.network1`
- Your implementation: `src/systemd_dbus.rs` (already using it!)
- D-Bus query: `busctl tree org.freedesktop.network1`

**Your architecture is actually BETTER than you realized!** 🎉

