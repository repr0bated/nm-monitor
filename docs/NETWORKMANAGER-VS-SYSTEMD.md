# NetworkManager vs systemd-networkd - Why We Switched

**TL;DR**: NetworkManager is great for **desktops**, terrible for **servers/VPS**. You made the right choice switching to systemd-networkd.

---

## 🖥️ **NetworkManager's Benefits** (Why it exists)

### ✅ What NetworkManager is GOOD at:

1. **Automatic WiFi Management**
   - Scan for networks
   - Store WiFi passwords
   - Auto-reconnect to known networks
   - Switch networks seamlessly

2. **GUI/Desktop Integration**
   - System tray applet
   - Click to connect to WiFi
   - VPN integration with GUI
   - User-friendly for non-technical users

3. **Dynamic Network Changes**
   - Laptop unplugged → switches to WiFi
   - Roaming between access points
   - Hotel captive portal detection
   - Mobile hotspot tethering

4. **Per-User VPN Connections**
   - VPN settings per user account
   - GUI VPN configuration
   - Multiple VPN profiles

5. **USB Ethernet/Tethering**
   - Plug in phone → auto-configure tethering
   - Plug in USB ethernet → auto-configure
   - Multiple adapters managed automatically

### 💻 **Perfect For:**
- Laptops
- Desktop workstations
- Developers who move between locations
- Users who want "it just works"

---

## ❌ **NetworkManager's Problems for Servers/VPS**

### 🔴 Why You Had Issues:

#### 1. **Race Conditions with OVS Bridges**
```bash
# NetworkManager tries to manage OVS bridges
# But OVS also manages them
# Result: Fight over who controls the bridge
```
**Your Experience**: "race issues, inconsistencies, unreliable"

**Why**: NetworkManager assumes it's the ONLY network manager. It doesn't play well with OVS, Docker, or other network tools.

#### 2. **Automatic "Helpfulness" is Harmful**
```bash
# NetworkManager sees a new interface
# Automatically tries to configure it
# Breaks your carefully configured OVS topology
```
**Example**: You create ovsbr0 → NetworkManager decides to "help" and reconfigures it.

#### 3. **State Conflicts**
```
NetworkManager State:  Interface UP, DHCP enabled
OVS State:            Interface is bridge port (no DHCP)
Actual State:         ???  (undefined behavior)
```

#### 4. **Unpredictable Reconnection**
- NetworkManager decides to "reconnect" your uplink
- Drops all bridge traffic momentarily
- No way to disable this "helpfulness"

#### 5. **Hidden Configuration**
```bash
# Where is the config?
/etc/NetworkManager/system-connections/
/var/lib/NetworkManager/
/run/NetworkManager/
# Good luck finding which file matters!
```

---

## ✅ **systemd-networkd's Benefits for Servers**

### Why Your Switch Was Correct:

#### 1. **Declarative Configuration**
```ini
# /etc/systemd/network/10-ovsbr0.network
[Match]
Name=ovsbr0

[Network]
DHCP=yes
```
**Simple, clear, one file per interface.**

#### 2. **No Automatic "Help"**
- Only manages what you tell it to
- Doesn't fight with OVS
- Doesn't "discover" and auto-configure

#### 3. **Predictable Behavior**
```bash
# Config file → Exact behavior
# No hidden state
# No race conditions
```

#### 4. **Fast Startup**
- Part of systemd (already running)
- No separate daemon to wait for
- Deterministic ordering

#### 5. **Server-Focused Design**
- Static configs
- Bridge support
- VLAN support
- Bonding/aggregation
- No GUI assumptions

---

## 📊 **Comparison for YOUR Use Case**

| Feature | NetworkManager | systemd-networkd | Winner |
|---------|----------------|------------------|--------|
| **Desktop/Laptop** | ✅ Excellent | ❌ Manual | NM |
| **Server/VPS** | ❌ Problematic | ✅ Excellent | networkd |
| **OVS Bridges** | ❌ Fights with OVS | ✅ Cooperates | networkd |
| **Docker Bridges** | ⚠️ Sometimes works | ✅ Works well | networkd |
| **Static IP** | ⚠️ GUI or nmcli | ✅ Config file | networkd |
| **Reproducibility** | ❌ Hidden state | ✅ Declarative | networkd |
| **Race Conditions** | ❌ Common | ✅ Rare | networkd |
| **WiFi** | ✅ Automatic | ❌ Manual (wpa_supplicant) | NM |
| **Startup Time** | ⚠️ Slow | ✅ Fast | networkd |
| **Configuration** | ⚠️ Complex | ✅ Simple | networkd |

---

## 🎯 **Why You Struggled with NetworkManager**

### Your Specific Issues:

```
Your Setup:
- OVS bridges (ovsbr0, ovsbr1)
- VPS with static IP (or DHCP from provider)
- Docker integration
- Netmaker networking
- Container port management

NetworkManager's Behavior:
1. "Oh, I see ovsbr0! Let me manage that!"
2. "Let me configure DHCP on ovsbr0 and all its ports!"
3. "Wait, enxe04f43a07fce is enslaved? Let me free it!"
4. "Connection lost? Let me auto-reconnect (drops traffic)!"
5. "New veth interface? Let me add that to my management!"
```

**Result**: Race conditions, dropped connections, unpredictable state

### What You Said:
> "NetworkManager has been a major problem/headache with race issues, inconsistencies, unreliable. I've been struggling since before January."

**This is EXACTLY what NetworkManager does on servers with OVS!**

---

## ✅ **Why systemd-networkd Solves Your Problems**

### 1. No More Race Conditions
```ini
# You configure ovsbr0
[Match]
Name=ovsbr0

# systemd-networkd does ONLY what you say
# Doesn't fight with OVS
# Doesn't auto-reconfigure
```

### 2. Predictable Behavior
```bash
# Config file → Behavior
# No surprises
# No "helping"
```

### 3. Works with OVS
```bash
# OVS creates the bridge
ovs-vsctl add-br ovsbr0

# systemd-networkd configures IP
# No conflict!
```

### 4. Docker Integration
```bash
# Docker can use ovsbr1
# systemd-networkd doesn't interfere
# Clean separation
```

---

## 🤔 **So What WAS NetworkManager Good For?**

### NetworkManager Shines On:
- **Your Laptop**: WiFi roaming, VPN switching, hotel WiFi
- **Desktop Workstation**: Easy network configuration GUI
- **Developer Machine**: USB tethering, multiple networks

### NetworkManager FAILS On:
- **Servers**: Static config, no need for dynamic changes
- **VPS**: Single uplink, needs reliability over flexibility
- **Container Hosts**: Docker/OVS need control, not "help"
- **Your Use Case**: OVS bridges, static topology, production

---

## 🎯 **CONCLUSION**

### Why You Switched:
✅ **You made the right choice!**

NetworkManager's benefits (WiFi, GUI, dynamic switching) **don't matter** for your VPS/server use case.

NetworkManager's problems (race conditions, auto-configuration, complexity) **do matter** and were blocking you.

### What You Gained:
- ✅ No more race conditions
- ✅ Predictable behavior
- ✅ OVS compatibility
- ✅ Simple configuration
- ✅ Fast startup
- ✅ Reproducible state

### What You Lost:
- ❌ WiFi auto-management (you don't use WiFi on VPS)
- ❌ GUI configuration (you don't need GUI on server)
- ❌ Auto-discovery (you don't want auto-config on server)

**Nothing of value was lost!**

---

## 📝 **Historical Context**

### NetworkManager Was Created For:
- GNOME desktop integration
- Laptop users who roam between networks
- Non-technical users who want "it just works"
- Era: ~2004 when laptops needed WiFi management

### systemd-networkd Was Created For:
- Servers and embedded systems
- Static, predictable configurations
- Fast boot times
- Integration with systemd
- Era: ~2014 when systemd unified Linux init

**Different tools for different jobs!**

---

## 🚀 **Your Path Forward**

### Keep Using systemd-networkd For:
- ✅ VPS networking
- ✅ OVS bridge management
- ✅ Docker integration
- ✅ Production deployments
- ✅ Container networking

### Use NetworkManager For:
- 💻 Your laptop (if you have one)
- 🖥️ Your desktop workstation
- 📱 USB tethering/mobile hotspot
- 🏨 WiFi roaming

---

## 💡 **Final Thought**

> **"NetworkManager is like automatic transmission - great for city driving (desktops), but you want manual transmission (systemd-networkd) for racing (servers)."**

You've been struggling because you were using the wrong tool for the job. Now you have the right tool (systemd-networkd) and the right abstraction (StateManager).

**Problem solved!** 🎉

---

**References:**
- Your struggle: "been struggling since before January"
- NetworkManager issues: "race issues, inconsistencies, unreliable"
- Solution: systemd-networkd + StateManager = **Declarative, reliable networking**

