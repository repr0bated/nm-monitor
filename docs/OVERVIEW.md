# nm-monitor Documentation Index

## 📚 Documentation Suite

This directory contains comprehensive documentation for the nm-monitor (OVS Port Agent) system.

---

## 🎯 Start Here

### For System Administrators & DevOps Teams

1. **[DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md](DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md)** ⭐ **START HERE**
   - Complete deployment guide specific to nm-monitor
   - Explains state management architecture
   - Shows how D-Bus, systemd-networkd, OVS interact
   - Why state management is critical
   - Step-by-step deployment procedures
   - Operational commands and troubleshooting

2. **[NETWORK_COMPONENTS_INTERACTION_GUIDE.md](NETWORK_COMPONENTS_INTERACTION_GUIDE.md)**
   - General reference for D-Bus, systemd-networkd, OVS
   - 80+ detailed commands for daily administration
   - Troubleshooting scenarios
   - Security hardening

### For End Users

- Start with **[DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md](DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md)** - Executive Summary section
- Focus on "Operational Commands" section for daily use

---

## 📖 What's Where

### Architecture & Concepts

**[DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md](DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md)**
- Architecture diagrams with data flow
- Component interaction patterns
- State lifecycle explanation
- Why atomicity matters
- Checkpoint and rollback mechanisms

**Key Topics:**
```
├─ Executive Summary (what is nm-monitor?)
├─ Architecture Overview (system layers)
├─ Why State Management is Critical ⭐
├─ Component Relationships ⭐
├─ State Lifecycle ⭐
├─ Deployment Procedures
├─ Operational Commands
├─ Troubleshooting
└─ Advanced Topics
```

### Component References

**[NETWORK_COMPONENTS_INTERACTION_GUIDE.md](NETWORK_COMPONENTS_INTERACTION_GUIDE.md)**
- Deep dive into D-Bus commands
- systemd-networkd configuration
- Open vSwitch operations
- /etc/network/interfaces syntax

**Key Topics:**
```
├─ D-Bus Commands (20+ examples)
├─ systemd-networkd Commands (20+ examples)
├─ Open vSwitch Commands (80+ examples)
├─ /etc/network/interfaces Examples
├─ Integration Workflows
├─ Migration Guides
├─ Performance Tuning
└─ Security Hardening
```

---

## 🚀 Quick Start Paths

### Path 1: Deploy on Production Server

```bash
# 1. Read deployment guide
cat docs/DEPLOYMENT_GUIDE_STATE_MANAGEMENT.md

# 2. Install system
sudo ./scripts/install.sh --bridge ovsbr0 --uplink eth0 --system

# 3. Create initial state
cat > /opt/network-config/initial.yaml << EOF
version: 1
plugins:
  net:
    interfaces:
      - name: ovsbr0
        type: ovs-bridge
        ports: [eth0]
        ipv4:
          enabled: true
          dhcp: true
EOF

# 4. Apply state
sudo ovs-port-agent apply-state /opt/network-config/initial.yaml

# 5. Verify
sudo ovs-port-agent query-state
```

### Path 2: Understanding State Management

Read in this order:
1. **DEPLOYMENT_GUIDE** → "Why State Management is Critical" section
2. **DEPLOYMENT_GUIDE** → "Component Relationships" section
3. **DEPLOYMENT_GUIDE** → "State Lifecycle" section
4. **DEPLOYMENT_GUIDE** → "Deployment Procedures" section

### Path 3: Daily Operations

Reference:
1. **DEPLOYMENT_GUIDE** → "Operational Commands" section
2. **NETWORK_COMPONENTS_GUIDE** → Search for specific commands

---

## 🔍 Finding Information

### By Topic

| What You Need | Where to Find It |
|---------------|------------------|
| **Installation** | DEPLOYMENT_GUIDE → Deployment Procedures |
| **State file syntax** | DEPLOYMENT_GUIDE → Plugin Architecture |
| **D-Bus commands** | NETWORK_COMPONENTS_GUIDE → D-Bus Commands |
| **OVS commands** | NETWORK_COMPONENTS_GUIDE → Open vSwitch Commands |
| **Troubleshooting** | DEPLOYMENT_GUIDE → Troubleshooting |
| **systemd-networkd** | NETWORK_COMPONENTS_GUIDE → systemd-networkd Commands |
| **Why use this system** | DEPLOYMENT_GUIDE → Executive Summary |
| **Architecture** | DEPLOYMENT_GUIDE → Architecture Overview |
| **State lifecycle** | DEPLOYMENT_GUIDE → State Lifecycle |
| **Examples** | /git/nm-monitor/config/examples/ |

### By Role

#### System Administrator
**Must Read:**
1. DEPLOYMENT_GUIDE (entire document)
2. NETWORK_COMPONENTS_GUIDE (reference as needed)

**Focus Areas:**
- Deployment procedures
- Operational commands
- Troubleshooting
- Monitoring

#### DevOps Engineer
**Must Read:**
1. DEPLOYMENT_GUIDE → Architecture & State Management sections
2. DEPLOYMENT_GUIDE → Advanced Topics

**Focus Areas:**
- State file format
- Automation integration
- CI/CD pipelines
- Monitoring and alerting

#### Network Engineer
**Must Read:**
1. DEPLOYMENT_GUIDE → Component Relationships
2. NETWORK_COMPONENTS_GUIDE (entire document)

**Focus Areas:**
- OVS configuration
- systemd-networkd integration
- Network architecture
- Performance tuning

#### End User (Operators)
**Must Read:**
1. DEPLOYMENT_GUIDE → Executive Summary
2. DEPLOYMENT_GUIDE → Operational Commands

**Focus Areas:**
- Query system state
- Apply configuration changes
- Basic troubleshooting
- Reading logs

---

## 📊 Architecture at a Glance

```
YAML State File → D-Bus RPC → State Manager → Plugins → systemd-networkd → OVS → Kernel
       ↓                                         ↓
   Desired State                           Blockchain Ledger
       ↓                                    (Immutable Audit)
   Diff Calculation
       ↓
   Atomic Apply
       ↓
   Verification
       ↓
   Success or Rollback
```

**Key Concepts:**
1. **Declarative** - Describe what you want, not how to do it
2. **Atomic** - All changes succeed or none do
3. **Auditable** - Blockchain ledger tracks everything
4. **Safe** - Automatic rollback on failure
5. **Idempotent** - Apply same state multiple times safely

---

## 🔧 Configuration Examples

Located in: `/git/nm-monitor/config/examples/`

| File | Description |
|------|-------------|
| `network-ovsbr0-only.yaml` | Single OVS bridge with uplink |
| `full-stack.yaml` | Complete network + configuration |
| `network-static-ip.yaml` | Static IP configuration |
| `netcfg-routing.yaml` | Custom routing rules |
| `netcfg-ovs-flows.yaml` | OVS flow rules |

---

## 🐛 Troubleshooting Quick Reference

### Issue → Solution

| Problem | Document | Section |
|---------|----------|---------|
| State apply fails | DEPLOYMENT_GUIDE | Troubleshooting → Issue 1 |
| Network lost | DEPLOYMENT_GUIDE | Troubleshooting → Issue 2 |
| D-Bus not working | DEPLOYMENT_GUIDE | Troubleshooting → Issue 3 |
| OVS bridge issues | NETWORK_COMPONENTS_GUIDE | Troubleshooting |
| systemd-networkd issues | NETWORK_COMPONENTS_GUIDE | systemd-networkd section |

### Quick Debug Commands

```bash
# Check service status
systemctl status ovs-port-agent

# View recent logs
journalctl -u ovs-port-agent -n 100

# Query current state
sudo ovs-port-agent query-state

# Verify blockchain
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.verify_blockchain_integrity

# Check network connectivity
ip addr show
ovs-vsctl show
networkctl list
```

---

## 📖 Additional Resources

### In This Repository

- **README.md** - Project overview and quickstart
- **DBUS_BLOCKCHAIN.md** - Blockchain ledger deep dive
- **PROXMOX_DEPLOYMENT.md** - Proxmox-specific deployment
- **config/examples/README.md** - State file examples

### External References

- **systemd-networkd**: https://www.freedesktop.org/software/systemd/man/systemd.network.html
- **Open vSwitch**: http://docs.openvswitch.org/
- **D-Bus**: https://www.freedesktop.org/wiki/Software/dbus/

---

## 🎓 Learning Path

### Beginner (First Week)

1. Read: DEPLOYMENT_GUIDE → Executive Summary
2. Read: DEPLOYMENT_GUIDE → "Why State Management is Critical"
3. Try: Deploy on test VM
4. Practice: Apply simple state changes
5. Learn: Query state and view blockchain

### Intermediate (First Month)

1. Read: DEPLOYMENT_GUIDE → Component Relationships
2. Read: DEPLOYMENT_GUIDE → State Lifecycle
3. Try: Complex multi-bridge configurations
4. Practice: Troubleshooting common issues
5. Learn: Write custom state files

### Advanced (Ongoing)

1. Read: DEPLOYMENT_GUIDE → Advanced Topics
2. Read: NETWORK_COMPONENTS_GUIDE (full)
3. Try: Custom plugin development
4. Practice: HA configurations
5. Learn: Integration with external systems

---

## 💡 Key Insights

### Understanding State Management

**Traditional Approach:**
```bash
ovs-vsctl add-br ovsbr0          # Command 1
ovs-vsctl add-port ovsbr0 eth0   # Command 2
ip addr add 192.168.1.100/24 ... # Command 3
# If Command 3 fails → BROKEN STATE 😱
```

**nm-monitor Approach:**
```yaml
# Describe desired state
version: 1
plugins:
  net:
    interfaces:
      - name: ovsbr0
        type: ovs-bridge
        # ...
```
```bash
# System atomically applies (or rolls back)
sudo ovs-port-agent apply-state state.yaml
# ✅ Either fully applied or fully rolled back
```

### Why This Matters

1. **Zero Downtime** - Atomic operations prevent broken states
2. **Complete Audit** - Blockchain tracks every change
3. **Safe Experimentation** - Automatic rollback on failure
4. **Predictable Results** - Idempotent operations
5. **Easy Automation** - Declarative configuration files

---

## 📞 Getting Help

### Log Analysis

```bash
# Service logs
journalctl -u ovs-port-agent -f

# With priority filtering
journalctl -u ovs-port-agent -p err -n 50

# Blockchain inspection
sudo tail -f /var/lib/ovs-port-agent/ledger.jsonl
```

### State Inspection

```bash
# Current network state
sudo ovs-port-agent query-state --plugin net

# Show what would change
sudo ovs-port-agent show-diff new-state.yaml

# Blockchain history
dbus-send --system --print-reply \
  --dest=dev.ovs.PortAgent1 \
  /dev/ovs/PortAgent1 \
  dev.ovs.PortAgent1.get_blocks_by_category \
  string:"network"
```

---

## 🔄 Document Updates

This documentation is actively maintained. Last major update: 2025-10-14

### Changelog

- **2025-10-14**: Created comprehensive deployment guide with state management focus
- **2025-10-14**: Added general network components interaction guide

---

## ✅ Checklist: Am I Ready for Production?

- [ ] Read DEPLOYMENT_GUIDE (at least Executive Summary + Deployment Procedures)
- [ ] Tested deployment on staging/test environment
- [ ] Created production state files
- [ ] Verified state files with `show-diff`
- [ ] Backup existing network configuration
- [ ] Tested rollback procedures
- [ ] Monitoring/alerting configured
- [ ] Team trained on operational commands
- [ ] Troubleshooting procedures documented
- [ ] Blockchain backup strategy defined

---

**Happy Deploying! 🚀**

For questions or issues, review the troubleshooting sections in both guides.
