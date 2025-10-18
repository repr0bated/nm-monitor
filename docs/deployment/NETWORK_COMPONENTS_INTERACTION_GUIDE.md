# Network Components Interaction Guide: D-Bus, systemd-networkd, Open vSwitch, and /etc/network/interfaces

**Comprehensive Technical Documentation for System Administrators, DevOps Engineers, and Network Engineers**

---

## Table of Contents

1. [Overview of Each Component](#overview-of-each-component)
2. [How They Interact (and Conflict)](#how-they-interact-and-conflict)
3. [Detailed Commands for Everyday Administration](#detailed-commands-for-everyday-administration)
4. [Integration Commands & Workflows](#integration-commands--workflows)
5. [Best Practices](#best-practices)
6. [Common Scenarios](#common-scenarios)

---

## Overview of Each Component

### 1. **D-Bus (Desktop Bus)**

D-Bus is an inter-process communication (IPC) mechanism that allows different processes and system services to communicate with each other.

**Role in networking:**
- Provides a message bus for network management services to communicate
- NetworkManager, systemd-networkd, and other services can send/receive events via D-Bus
- Used for signaling network state changes, connection events, and configuration updates

**Architecture:**
```
┌─────────────────────────────────────┐
│        System D-Bus Daemon          │
│    (org.freedesktop.DBus)           │
└─────────────────────────────────────┘
         ↑              ↑              ↑
         │              │              │
  ┌──────┴──┐    ┌──────┴──┐    ┌─────┴──┐
  │NetworkMgr│    │systemd- │    │  OVS   │
  │         │    │networkd │    │ Agent  │
  └─────────┘    └─────────┘    └────────┘
```

### 2. **systemd-networkd**

A system service that manages network configurations in systemd-based systems.

**Key characteristics:**
- Manages wired and wireless connections
- Reads configuration from `/etc/systemd/network/*.network` files
- Lightweight alternative to NetworkManager
- Directly integrated with systemd
- Uses D-Bus for IPC

**Configuration Files:**
- `.network` - Network configuration (IP, routes, DNS)
- `.netdev` - Virtual device creation (VLAN, bridge, bond)
- `.link` - Link configuration (MAC, MTU, name)

### 3. **Open vSwitch (OVS)**

A production-grade virtual switch implementation.

**Key characteristics:**
- Creates virtual bridges and ports
- Supports VLANs, QoS, tunneling protocols (VXLAN, GRE, etc.)
- Has its own database (OVSDB) for configuration
- Independent of traditional Linux networking tools
- OpenFlow-capable for SDN integration

**Architecture:**
```
┌──────────────────────────────────────┐
│      ovs-vswitchd (datapath)         │
└──────────────────────────────────────┘
              ↕
┌──────────────────────────────────────┐
│        ovsdb-server (config)         │
└──────────────────────────────────────┘
              ↕
┌──────────────────────────────────────┐
│    ovs-vsctl (management CLI)        │
└──────────────────────────────────────┘
```

### 4. **/etc/network/interfaces**

Traditional Debian/Ubuntu network configuration file used by the `ifupdown` package.

**Key characteristics:**
- Legacy configuration method
- Managed by `ifup`/`ifdown` commands
- Still widely used, especially in older systems
- Simple, human-readable syntax

---

## How They Interact (and Conflict)

### **Potential Conflicts**

These systems can **conflict** because they all try to manage the same network interfaces:

```
┌─────────────────────────────────────────┐
│      Network Interface (eth0)           │
└─────────────────────────────────────────┘
           ↑         ↑         ↑
           │         │         │
    ┌──────┴──┐ ┌────┴────┐ ┌─┴────────┐
    │ ifupdown│ │systemd- │ │   OVS    │
    │         │ │networkd │ │          │
    └─────────┘ └─────────┘ └──────────┘
```

**Common issues:**
- Multiple services trying to configure the same interface
- Race conditions during boot
- Conflicting IP address assignments
- Interface state inconsistencies
- DHCP client conflicts (multiple DHCP clients running)

### **Interaction Patterns**

#### **D-Bus Communication**

```bash
# systemd-networkd can emit D-Bus signals for network events
# Listen to networkd D-Bus messages:
dbus-monitor --system "type='signal',sender='org.freedesktop.network1'"

# Check if systemd-networkd is registered on D-Bus:
busctl list | grep networkd

# Example output:
# org.freedesktop.network1    - -    systemd-networkd
```

**Key D-Bus interfaces:**
- `org.freedesktop.network1.Manager` - Network management
- `org.freedesktop.network1.Link` - Link-layer configuration
- `org.freedesktop.network1.Network` - Network-layer configuration

#### **systemd-networkd ↔ OVS**

- systemd-networkd can bring up OVS ports as regular interfaces
- OVS must be configured separately through `ovs-vsctl`
- systemd-networkd sees OVS ports as regular interfaces

Example configuration:
```ini
# /etc/systemd/network/10-ovs-port.network
[Match]
Name=vport0

[Network]
Address=192.168.100.10/24
Gateway=192.168.100.1
```

#### **/etc/network/interfaces ↔ OVS**

OVS provides hooks for `/etc/network/interfaces`:

```bash
# /etc/network/interfaces
auto br0
iface br0 inet static
    address 192.168.1.10
    netmask 255.255.255.0
    ovs_type OVSBridge
    ovs_ports eth0 vport0
```

---

## Detailed Commands for Everyday Administration

### **D-Bus Commands**

#### **Basic D-Bus Operations**

```bash
# 1. List all D-Bus services
busctl list

# 2. List only network-related services
busctl list | grep -E 'network|NetworkManager'

# 3. Check systemd-networkd D-Bus interface
busctl tree org.freedesktop.network1

# 4. Get networkd properties via D-Bus
busctl get-property org.freedesktop.network1 \
    /org/freedesktop/network1 \
    org.freedesktop.network1.Manager \
    OperationalState

# 5. Monitor network-related D-Bus messages
dbus-monitor --system "interface='org.freedesktop.network1.Manager'"

# 6. Call a method via D-Bus (example: reload networkd config)
busctl call org.freedesktop.network1 \
    /org/freedesktop/network1 \
    org.freedesktop.network1.Manager \
    Reload

# 7. Introspect D-Bus interfaces (shows available methods/properties)
busctl introspect org.freedesktop.network1 \
    /org/freedesktop/network1

# 8. Show all links managed by networkd
busctl call org.freedesktop.network1 \
    /org/freedesktop/network1 \
    org.freedesktop.network1.Manager \
    ListLinks
```

#### **Advanced D-Bus Operations**

```bash
# 9. Get detailed link information
busctl introspect org.freedesktop.network1 \
    /org/freedesktop/network1/link/_32

# 10. Monitor all system D-Bus messages (verbose)
dbus-monitor --system

# 11. Check D-Bus policy for networkd
cat /usr/share/dbus-1/system.d/org.freedesktop.network1.conf

# 12. Send custom D-Bus signal (advanced)
dbus-send --system --type=signal \
    /org/freedesktop/network1 \
    org.freedesktop.network1.Manager.PropertiesChanged

# 13. Query D-Bus for network state changes
gdbus monitor --system --dest org.freedesktop.network1

# 14. Get all properties of a link
gdbus call --system \
    --dest org.freedesktop.network1 \
    --object-path /org/freedesktop/network1/link/_32 \
    --method org.freedesktop.DBus.Properties.GetAll \
    org.freedesktop.network1.Link
```

#### **D-Bus Debugging**

```bash
# 15. Check D-Bus daemon status
systemctl status dbus

# 16. Restart D-Bus (careful - may disconnect services)
# Not recommended unless necessary
sudo systemctl restart dbus

# 17. Check D-Bus logs
journalctl -u dbus -f

# 18. Validate D-Bus configuration
dbus-daemon --config-file=/etc/dbus-1/system.conf --check

# 19. List all D-Bus interfaces available
busctl tree

# 20. Check D-Bus security policies
ls -la /etc/dbus-1/system.d/
```

### **systemd-networkd Commands**

#### **Service Management**

```bash
# 1. Start/stop/restart systemd-networkd
systemctl start systemd-networkd
systemctl stop systemd-networkd
systemctl restart systemd-networkd

# 2. Check status with detailed information
systemctl status systemd-networkd -l

# 3. View networkd logs (real-time)
journalctl -u systemd-networkd -f

# 4. View networkd logs (last 100 lines)
journalctl -u systemd-networkd -n 100

# 5. View networkd logs since specific time
journalctl -u systemd-networkd --since "1 hour ago"

# 6. Enable/disable systemd-networkd at boot
systemctl enable systemd-networkd
systemctl disable systemd-networkd

# 7. Check if networkd is enabled
systemctl is-enabled systemd-networkd

# 8. Reload networkd configuration
systemctl reload systemd-networkd
```

#### **Network Control**

```bash
# 9. List all network links
networkctl list

# 10. Show detailed status of an interface
networkctl status eth0

# 11. Show all network configuration
networkctl status

# 12. Reload configuration without restarting
networkctl reload

# 13. Reconfigure a specific interface
networkctl reconfigure eth0

# 14. Force interface renewal (for DHCP)
networkctl renew eth0

# 15. Check if networkd is managing an interface
networkctl list eth0

# 16. Show link-layer information
networkctl lldp

# 17. View DHCP leases
networkctl lldp
cat /run/systemd/netif/leases/*
```

#### **Configuration Management**

```bash
# 18. List network configuration files
ls -la /etc/systemd/network/

# 19. Validate network configuration syntax
# (no built-in validator, but can check with systemd-analyze)
systemd-analyze verify systemd-networkd.service

# 20. Show which configuration files are loaded
systemctl cat systemd-networkd

# 21. Check network file permissions
ls -la /run/systemd/netif/
```

#### **Configuration File Examples**

**Static IP Configuration:**
```ini
# /etc/systemd/network/10-wired.network
[Match]
Name=eth0

[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
DNS=8.8.8.8
DNS=8.8.4.4

[Route]
Gateway=192.168.1.1
Destination=10.0.0.0/8
```

**DHCP Configuration:**
```ini
# /etc/systemd/network/20-dhcp.network
[Match]
Name=eth1

[Network]
DHCP=yes

[DHCP]
UseDNS=yes
UseRoutes=yes
RouteMetric=100
```

**Bridge Configuration:**
```ini
# /etc/systemd/network/30-bridge.netdev
[NetDev]
Name=br0
Kind=bridge

# /etc/systemd/network/30-bridge-bind.network
[Match]
Name=eth0

[Network]
Bridge=br0

# /etc/systemd/network/30-bridge.network
[Match]
Name=br0

[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
```

**VLAN Configuration:**
```ini
# /etc/systemd/network/40-vlan10.netdev
[NetDev]
Name=vlan10
Kind=vlan

[VLAN]
Id=10

# /etc/systemd/network/40-vlan10.network
[Match]
Name=vlan10

[Network]
Address=192.168.10.1/24
```

**Bond Configuration:**
```ini
# /etc/systemd/network/50-bond0.netdev
[NetDev]
Name=bond0
Kind=bond

[Bond]
Mode=802.3ad
MIIMonitorSec=1s

# /etc/systemd/network/50-bond0-bind.network
[Match]
Name=eth0 eth1

[Network]
Bond=bond0

# /etc/systemd/network/50-bond0.network
[Match]
Name=bond0

[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
```

### **Open vSwitch Commands**

#### **Service Management**

```bash
# 1. Check OVS service status
systemctl status openvswitch-switch
# or on some systems:
systemctl status ovs-vswitchd
systemctl status ovsdb-server

# 2. Start/stop/restart OVS services
systemctl start openvswitch-switch
systemctl restart openvswitch-switch

# 3. Show OVS version
ovs-vsctl --version
ovs-ofctl --version

# 4. Check OVS database connection
ovs-vsctl show
```

#### **Bridge Operations**

```bash
# 5. Show all bridges
ovs-vsctl list-br

# 6. Show all ports on a bridge
ovs-vsctl list-ports br0

# 7. Show detailed bridge information
ovs-vsctl show

# 8. Create a new bridge
ovs-vsctl add-br br0

# 9. Delete a bridge
ovs-vsctl del-br br0

# 10. Add a port to a bridge
ovs-vsctl add-port br0 eth0

# 11. Remove a port from a bridge
ovs-vsctl del-port br0 eth0

# 12. Create an internal port (virtual interface)
ovs-vsctl add-port br0 vport0 -- set interface vport0 type=internal

# 13. Check if port exists on bridge
ovs-vsctl port-to-br eth0

# 14. List all interfaces on a port
ovs-vsctl list-ifaces br0

# 15. Get bridge datapath ID
ovs-vsctl get bridge br0 datapath-id
```

#### **VLAN Operations**

```bash
# 16. Set port as VLAN trunk (allows multiple VLANs)
ovs-vsctl set port eth0 trunks=10,20,30

# 17. Set port as VLAN access port (single VLAN)
ovs-vsctl set port eth0 tag=100

# 18. Remove VLAN tag from port
ovs-vsctl remove port eth0 tag 100

# 19. Show port VLAN configuration
ovs-vsctl get port eth0 tag
ovs-vsctl get port eth0 trunks

# 20. Set native VLAN (for untagged traffic on trunk)
ovs-vsctl set port eth0 vlan_mode=native-untagged
ovs-vsctl set port eth0 tag=1
```

#### **Port and Interface Configuration**

```bash
# 21. Show port configuration
ovs-vsctl list port eth0

# 22. Show interface configuration
ovs-vsctl list interface eth0

# 23. Set interface MTU
ovs-vsctl set interface eth0 mtu_request=9000

# 24. Set port in promiscuous mode
ovs-vsctl set interface eth0 options:promiscuous=true

# 25. Get port statistics
ovs-vsctl get interface eth0 statistics

# 26. Clear port statistics
ovs-vsctl clear interface eth0 statistics
```

#### **OpenFlow Operations**

```bash
# 27. Show OpenFlow version
ovs-ofctl --version

# 28. Show OpenFlow ports
ovs-ofctl show br0

# 29. Show flow table
ovs-ofctl dump-flows br0

# 30. Clear all flows (careful!)
ovs-ofctl del-flows br0

# 31. Add a flow rule
ovs-ofctl add-flow br0 "priority=100,in_port=1,actions=output:2"

# 32. Add flow with specific match
ovs-ofctl add-flow br0 \
    "priority=200,ip,nw_src=192.168.1.0/24,actions=normal"

# 33. Delete specific flows
ovs-ofctl del-flows br0 "in_port=1"

# 34. Show flow statistics
ovs-ofctl dump-flows br0 --rsort

# 35. Show aggregate statistics
ovs-ofctl dump-aggregate br0
```

#### **Advanced Features**

```bash
# 36. Set bridge controller (for SDN)
ovs-vsctl set-controller br0 tcp:192.168.1.1:6653

# 37. Show controller configuration
ovs-vsctl get-controller br0

# 38. Remove controller (standalone mode)
ovs-vsctl del-controller br0

# 39. Set bridge to fail-secure mode
ovs-vsctl set bridge br0 fail-mode=secure

# 40. Set bridge to fail-standalone mode
ovs-vsctl set bridge br0 fail-mode=standalone

# 41. Enable STP (Spanning Tree Protocol)
ovs-vsctl set bridge br0 stp_enable=true

# 42. Disable STP
ovs-vsctl set bridge br0 stp_enable=false

# 43. Show STP status
ovs-appctl stp/show br0

# 44. Set bridge protocols (OpenFlow versions)
ovs-vsctl set bridge br0 protocols=OpenFlow10,OpenFlow13
```

#### **Port Mirroring**

```bash
# 45. Configure port mirroring (span)
ovs-vsctl -- set Bridge br0 mirrors=@m \
    -- --id=@eth0 get Port eth0 \
    -- --id=@eth1 get Port eth1 \
    -- --id=@m create Mirror name=mirror0 \
       select-dst-port=@eth0 \
       select-src-port=@eth0 \
       output-port=@eth1

# 46. List mirrors
ovs-vsctl list mirror

# 47. Remove mirror
ovs-vsctl clear bridge br0 mirrors

# 48. Show mirror configuration
ovs-vsctl get mirror mirror0 select_all
```

#### **QoS and Rate Limiting**

```bash
# 49. Set QoS on a port
ovs-vsctl set port eth0 qos=@newqos \
    -- --id=@newqos create qos type=linux-htb \
    other-config:max-rate=1000000000

# 50. Set ingress policing (rate limiting)
ovs-vsctl set interface eth0 ingress_policing_rate=1000
ovs-vsctl set interface eth0 ingress_policing_burst=100

# 51. Show QoS configuration
ovs-vsctl list qos

# 52. Remove QoS from port
ovs-vsctl destroy qos <qos-uuid>
ovs-vsctl clear port eth0 qos
```

#### **Bonding/Link Aggregation**

```bash
# 53. Create bond (LACP/802.3ad)
ovs-vsctl add-bond br0 bond0 eth0 eth1 \
    bond_mode=balance-slb

# 54. Create LACP bond
ovs-vsctl add-bond br0 bond0 eth0 eth1 \
    lacp=active

# 55. Show bond status
ovs-appctl bond/show bond0

# 56. Show LACP status
ovs-appctl lacp/show bond0

# 57. Set bond mode
ovs-vsctl set port bond0 bond_mode=balance-tcp

# 58. Remove bond
ovs-vsctl del-port br0 bond0
```

#### **Tunneling**

```bash
# 59. Create VXLAN tunnel
ovs-vsctl add-port br0 vxlan1 \
    -- set interface vxlan1 type=vxlan \
       options:remote_ip=192.168.1.100 \
       options:key=1000

# 60. Create GRE tunnel
ovs-vsctl add-port br0 gre1 \
    -- set interface gre1 type=gre \
       options:remote_ip=192.168.1.100

# 61. Show tunnel configuration
ovs-vsctl list interface vxlan1

# 62. Remove tunnel
ovs-vsctl del-port br0 vxlan1
```

#### **Database Operations**

```bash
# 63. Show OVS database schema
ovsdb-client get-schema unix:/var/run/openvswitch/db.sock Open_vSwitch

# 64. Dump entire OVS database
ovsdb-client dump

# 65. Backup OVS configuration
ovsdb-client backup > ovs-backup.db

# 66. Query specific table
ovsdb-client transact '["Open_vSwitch",
    {"op":"select","table":"Bridge"}]'

# 67. Monitor database changes
ovsdb-client monitor Bridge
```

#### **Datapath Operations**

```bash
# 68. Show datapath information
ovs-dpctl show

# 69. Show datapath flows
ovs-dpctl dump-flows

# 70. Delete datapath flows
ovs-dpctl del-flows

# 71. Show datapath statistics
ovs-dpctl show -s
```

#### **Debugging and Diagnostics**

```bash
# 72. Show coverage statistics
ovs-appctl coverage/show

# 73. Show memory usage
ovs-appctl memory/show

# 74. Show vlog (logging) configuration
ovs-appctl vlog/list

# 75. Change log level
ovs-appctl vlog/set ANY:file:dbg

# 76. Trace packet through bridge
ovs-appctl ofproto/trace br0 \
    in_port=1,dl_src=00:00:00:00:00:01,dl_dst=00:00:00:00:00:02

# 77. Show OpenFlow connection status
ovs-appctl bridge/dump-flows br0

# 78. Show OVS logs
journalctl -u openvswitch-switch -f
journalctl -u ovs-vswitchd -f
journalctl -u ovsdb-server -f

# 79. Test connectivity through OVS
ovs-appctl netdev-dummy/receive <port> <packet>

# 80. Show all OVS processes
ps aux | grep ovs
```

### **/etc/network/interfaces Commands**

#### **Basic Operations**

```bash
# 1. Bring up an interface
ifup eth0

# 2. Bring down an interface
ifdown eth0

# 3. Bring up all interfaces marked as 'auto'
ifup -a

# 4. Bring down all interfaces
ifdown -a

# 5. Force bring down (ignore errors)
ifdown --force eth0

# 6. Show configuration that would be applied (dry-run)
ifup --no-act eth0
ifup -n eth0

# 7. Verbose output
ifup -v eth0

# 8. Force reconfiguration (down then up)
ifdown eth0 && ifup eth0

# 9. Reload configuration and apply
ifdown -a && ifup -a

# 10. Bring up with specific configuration
ifup eth0=static-config
```

#### **Query and Information**

```bash
# 11. Check if interface is configured in interfaces file
grep -A 10 "iface eth0" /etc/network/interfaces

# 12. Show all configured interfaces
grep "^iface" /etc/network/interfaces

# 13. Show which interfaces are managed by ifupdown
ifquery --list

# 14. Show configuration for a specific interface
ifquery eth0

# 15. Show current state of interface
ifquery --state eth0

# 16. Show all interface states
ifquery --state

# 17. Validate configuration syntax (manual)
cat /etc/network/interfaces

# 18. Show which config file an interface uses
ifquery -l eth0
```

#### **Advanced Operations**

```bash
# 19. Include configuration from directory
# Add to /etc/network/interfaces:
# source /etc/network/interfaces.d/*

# 20. Use alternative configuration file
ifup -i /etc/network/interfaces.backup eth0

# 21. Exclude interfaces from ifup -a
# Add to interface config: auto eth0
# Remove to exclude

# 22. Allow hotplug interfaces
# Replace 'auto' with 'allow-hotplug'
```

#### **Configuration File Examples**

**Static IP Configuration:**
```bash
# /etc/network/interfaces

auto eth0
iface eth0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    gateway 192.168.1.1
    dns-nameservers 8.8.8.8 8.8.4.4
    dns-search example.com
```

**DHCP Configuration:**
```bash
auto eth1
iface eth1 inet dhcp
    hostname myhostname
    # Request specific lease time
    request 192.168.1.150
```

**VLAN Configuration:**
```bash
auto eth0.10
iface eth0.10 inet static
    address 192.168.10.1
    netmask 255.255.255.0
    vlan-raw-device eth0
```

**Bridge Configuration (Linux bridge):**
```bash
auto br0
iface br0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    bridge_ports eth0 eth1
    bridge_stp off
    bridge_fd 0
    bridge_maxwait 0
```

**OVS Bridge with ifupdown:**
```bash
# Install ovs-vsctl-integration package first
auto br-ovs
allow-ovs br-ovs
iface br-ovs inet static
    address 192.168.1.100
    netmask 255.255.255.0
    ovs_type OVSBridge
    ovs_ports eth0 vport0

# OVS Port
allow-br-ovs eth0
iface eth0 inet manual
    ovs_bridge br-ovs
    ovs_type OVSPort

# OVS Internal Port
allow-br-ovs vport0
iface vport0 inet manual
    ovs_bridge br-ovs
    ovs_type OVSIntPort
```

**Bonding Configuration:**
```bash
# Install ifenslave package first
auto bond0
iface bond0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    bond-slaves eth0 eth1
    bond-mode 802.3ad
    bond-miimon 100
    bond-lacp-rate fast
    bond-xmit-hash-policy layer3+4
```

**Advanced Bonding Options:**
```bash
auto bond0
iface bond0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    bond-slaves eth0 eth1
    bond-mode active-backup
    bond-primary eth0
    bond-miimon 100
    bond-downdelay 200
    bond-updelay 200
```

**Multiple IP Addresses:**
```bash
auto eth0
iface eth0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    gateway 192.168.1.1

auto eth0:0
iface eth0:0 inet static
    address 192.168.1.101
    netmask 255.255.255.0

auto eth0:1
iface eth0:1 inet static
    address 192.168.1.102
    netmask 255.255.255.0
```

**Static Routes:**
```bash
auto eth0
iface eth0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    gateway 192.168.1.1
    # Add static routes
    up route add -net 10.0.0.0/8 gw 192.168.1.254
    down route del -net 10.0.0.0/8 gw 192.168.1.254
```

**Pre/Post Scripts:**
```bash
auto eth0
iface eth0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    pre-up /usr/local/bin/pre-eth0.sh
    post-up /usr/local/bin/post-eth0.sh
    pre-down /usr/local/bin/pre-down-eth0.sh
    post-down /usr/local/bin/post-down-eth0.sh
```

**Wireless Configuration:**
```bash
auto wlan0
iface wlan0 inet dhcp
    wpa-ssid "MyNetwork"
    wpa-psk "MyPassword"
```

---

## Integration Commands & Workflows

### **Checking What's Managing Your Network**

```bash
# 1. Check which network manager is active
systemctl is-active NetworkManager
systemctl is-active systemd-networkd
systemctl is-active networking  # ifupdown

# 2. Check if services are enabled at boot
systemctl is-enabled NetworkManager
systemctl is-enabled systemd-networkd
systemctl is-enabled networking

# 3. See all network-related services
systemctl list-units | grep -E 'network|ovs'

# 4. Check interface status from all perspectives
ip addr show
ip link show
networkctl status
ifquery --state
ovs-vsctl show

# 5. Check which service controls which interface
nmcli device status  # NetworkManager
networkctl list      # systemd-networkd
ifquery --list       # ifupdown

# 6. Check for DHCP clients running
ps aux | grep dhclient
ps aux | grep dhcpcd
systemctl status systemd-networkd  # has built-in DHCP

# 7. Check routing table
ip route show
route -n

# 8. Check DNS configuration
cat /etc/resolv.conf
resolvectl status  # if using systemd-resolved
```

### **Migrating Between Systems**

#### **From ifupdown to systemd-networkd:**

```bash
# 1. Backup existing configuration
cp /etc/network/interfaces /etc/network/interfaces.backup

# 2. Create equivalent .network files
# For static IP:
cat > /etc/systemd/network/10-eth0.network << EOF
[Match]
Name=eth0

[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
DNS=8.8.8.8
EOF

# 3. Disable networking service (ifupdown)
systemctl disable networking

# 4. Stop networking service
systemctl stop networking

# 5. Enable systemd-networkd
systemctl enable systemd-networkd
systemctl enable systemd-resolved

# 6. Start systemd-networkd
systemctl start systemd-networkd
systemctl start systemd-resolved

# 7. Verify connectivity
networkctl list
networkctl status eth0
ping -c 4 8.8.8.8

# 8. If issues occur, rollback
systemctl stop systemd-networkd
systemctl stop systemd-resolved
systemctl start networking
```

#### **From NetworkManager to systemd-networkd:**

```bash
# 1. Identify interfaces managed by NetworkManager
nmcli device status

# 2. Export NetworkManager connections (for reference)
for conn in $(nmcli -t -f NAME con show); do
    nmcli con show "$conn" > "/tmp/nm-$conn.conf"
done

# 3. Stop NetworkManager
systemctl stop NetworkManager

# 4. Disable NetworkManager
systemctl disable NetworkManager
systemctl mask NetworkManager  # prevent accidental start

# 5. Create systemd-networkd configuration
# Convert NetworkManager settings to .network files

# 6. Enable and start systemd-networkd
systemctl enable systemd-networkd
systemctl start systemd-networkd

# 7. Verify
networkctl list
```

#### **Setting up OVS with systemd-networkd:**

```bash
# 1. Install Open vSwitch
apt-get install openvswitch-switch

# 2. Create OVS bridge
ovs-vsctl add-br br0

# 3. Add physical port to bridge
ovs-vsctl add-port br0 eth0

# 4. Create internal port for management IP
ovs-vsctl add-port br0 br0-int -- set interface br0-int type=internal

# 5. Configure internal port with systemd-networkd
cat > /etc/systemd/network/10-ovs-br0.network << EOF
[Match]
Name=br0-int

[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
DNS=8.8.8.8
EOF

# 6. Ensure physical port has no IP
cat > /etc/systemd/network/20-eth0.network << EOF
[Match]
Name=eth0

[Network]
# No IP configuration - managed by OVS
LinkLocalAddressing=no
DHCP=no
EOF

# 7. Restart networkd
systemctl restart systemd-networkd

# 8. Bring up interface
ip link set br0 up
ip link set br0-int up

# 9. Verify
ovs-vsctl show
networkctl status br0-int
ping -c 4 8.8.8.8
```

#### **Setting up OVS with ifupdown:**

```bash
# 1. Install OVS and ifupdown integration
apt-get install openvswitch-switch

# 2. Configure in /etc/network/interfaces
cat >> /etc/network/interfaces << EOF

# OVS Bridge
auto br0
allow-ovs br0
iface br0 inet manual
    ovs_type OVSBridge
    ovs_ports eth0 br0-int

# Physical interface (no IP)
allow-br0 eth0
iface eth0 inet manual
    ovs_bridge br0
    ovs_type OVSPort

# Internal port (with IP)
allow-br0 br0-int
iface br0-int inet static
    ovs_bridge br0
    ovs_type OVSIntPort
    address 192.168.1.100
    netmask 255.255.255.0
    gateway 192.168.1.1
EOF

# 3. Bring up the bridge
ifup br0

# 4. Verify
ovs-vsctl show
ifconfig br0-int
```

### **Debugging Network Issues**

```bash
# 1. Check all interface states
ip link show
ip addr show

# 2. Check detailed interface information
ethtool eth0
ip -s link show eth0  # statistics

# 3. Check routing table
ip route show
ip route get 8.8.8.8

# 4. Check OVS bridge health
ovs-vsctl show
ovs-ofctl show br0
ovs-appctl bond/show  # if using bonding

# 5. Check systemd-networkd status
networkctl status
journalctl -u systemd-networkd -n 50 --no-pager

# 6. Check for conflicts
ps aux | grep -E 'NetworkManager|networkd|dhclient'

# 7. Test network connectivity
ping -c 4 8.8.8.8
ping -c 4 192.168.1.1  # gateway
traceroute 8.8.8.8

# 8. Check DNS resolution
resolvectl status
systemd-resolve --status
nslookup google.com
dig google.com

# 9. Check listening ports and connections
ss -tulpn
netstat -tulpn

# 10. Packet capture
tcpdump -i eth0 -n
tcpdump -i ovs-system -n  # For OVS

# 11. Check firewall rules
iptables -L -n -v
nft list ruleset

# 12. Check which process has a port
ss -tulpn | grep :80
lsof -i :80

# 13. Monitor D-Bus for network events
dbus-monitor --system | grep -i network

# 14. Check NetworkManager logs (if applicable)
journalctl -u NetworkManager -n 50

# 15. Check OVS logs
journalctl -u openvswitch-switch -f
journalctl -u ovs-vswitchd -f

# 16. Check kernel network messages
dmesg | grep -E 'eth|net|link'

# 17. Verify interface is not in promiscuous mode (unless intended)
ip link show eth0 | grep PROMISC

# 18. Check MTU settings
ip link show | grep mtu

# 19. Test TCP connection
nc -zv google.com 80
telnet google.com 80

# 20. Check ARP table
ip neigh show
arp -n
```

---

## Best Practices

### **1. Choose ONE Network Manager**

Don't run multiple network managers simultaneously to avoid conflicts:

```bash
# If using systemd-networkd:
systemctl disable NetworkManager
systemctl disable networking  # ifupdown
systemctl mask NetworkManager
systemctl stop NetworkManager

# If using NetworkManager:
systemctl disable systemd-networkd
systemctl disable networking
systemctl mask systemd-networkd

# If using ifupdown:
systemctl disable systemd-networkd
systemctl disable NetworkManager
systemctl mask systemd-networkd
systemctl mask NetworkManager
```

### **2. OVS Integration Strategy**

Use OVS for virtual networking, let your chosen network manager handle physical interfaces:

**Recommended approach:**
- **Physical interfaces**: Managed by systemd-networkd/NetworkManager/ifupdown
- **OVS bridges**: Created and managed by ovs-vsctl
- **OVS internal ports**: Get IP addresses from network manager
- **OVS flow rules**: Managed separately through ovs-ofctl

**Example workflow:**
```bash
# 1. Let network manager handle physical interface (no IP)
# 2. Create OVS bridge
ovs-vsctl add-br br0
# 3. Add physical interface to bridge
ovs-vsctl add-port br0 eth0
# 4. Create internal port
ovs-vsctl add-port br0 br0-mgmt -- set interface br0-mgmt type=internal
# 5. Assign IP to internal port via network manager
```

### **3. Configuration Precedence**

Understanding the order of evaluation:

```
1. Kernel command line parameters (boot time)
2. systemd-networkd (if active and enabled)
3. NetworkManager (if active and enabled)
4. ifupdown/interfaces file (if networking.service active)
5. Manual `ip` commands (temporary, lost on reboot)
```

**Priority matrix:**
| Priority | Method | Persistence | Complexity | Use Case |
|----------|--------|-------------|------------|----------|
| Highest  | Kernel params | Permanent | Low | Rescue/emergency |
| High     | systemd-networkd | Permanent | Medium | Modern systems |
| Medium   | NetworkManager | Permanent | High | Desktop/laptop |
| Low      | ifupdown | Permanent | Low | Legacy systems |
| Lowest   | Manual `ip` | Temporary | Low | Testing/debugging |

### **4. Persistent vs Temporary Changes**

```bash
# Temporary (lost on reboot or interface restart):
ip addr add 192.168.1.100/24 dev eth0
ip link set eth0 up
ip route add default via 192.168.1.1

# Persistent (survives reboot):
# Option 1: systemd-networkd
cat > /etc/systemd/network/10-eth0.network << EOF
[Match]
Name=eth0
[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
EOF
systemctl restart systemd-networkd

# Option 2: ifupdown
cat >> /etc/network/interfaces << EOF
auto eth0
iface eth0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    gateway 192.168.1.1
EOF
ifdown eth0 && ifup eth0

# Option 3: NetworkManager
nmcli con mod "Wired connection 1" ipv4.addresses 192.168.1.100/24
nmcli con mod "Wired connection 1" ipv4.gateway 192.168.1.1
nmcli con mod "Wired connection 1" ipv4.method manual
nmcli con up "Wired connection 1"
```

### **5. Backup Before Changes**

Always backup configuration before making changes:

```bash
# Backup systemd-networkd config
cp -r /etc/systemd/network /etc/systemd/network.backup

# Backup interfaces file
cp /etc/network/interfaces /etc/network/interfaces.backup

# Backup OVS database
ovsdb-client backup > /root/ovs-backup-$(date +%Y%m%d).db

# Backup NetworkManager connections
cp -r /etc/NetworkManager/system-connections \
    /etc/NetworkManager/system-connections.backup
```

### **6. Use Checkpoints for Risky Changes**

For NetworkManager and systemd, use checkpoints:

```bash
# NetworkManager checkpoint
nmcli general checkpoint create
# Make changes...
# If successful:
nmcli general checkpoint destroy <checkpoint-id>
# If failed:
nmcli general checkpoint rollback <checkpoint-id>

# For systemd-networkd, no native checkpoint
# Use manual backup/restore or Btrfs snapshots
```

### **7. Documentation and Comments**

Always document your configuration:

```bash
# systemd-networkd
cat > /etc/systemd/network/10-eth0.network << EOF
# Created: 2025-10-14
# Purpose: Main uplink interface
# Contact: admin@example.com
[Match]
Name=eth0
[Network]
Address=192.168.1.100/24
Gateway=192.168.1.1
EOF

# ifupdown
cat >> /etc/network/interfaces << EOF
# eth0 - Main uplink (added 2025-10-14)
auto eth0
iface eth0 inet static
    address 192.168.1.100
    netmask 255.255.255.0
    gateway 192.168.1.1
EOF
```

### **8. Monitoring and Alerting**

Set up monitoring for network changes:

```bash
# Monitor networkd via D-Bus
dbus-monitor --system "type='signal',sender='org.freedesktop.network1'" \
    | logger -t networkd-monitor &

# Monitor OVS changes
ovs-vsctl --columns=_uuid,name list Bridge | \
    while read line; do
        echo "[OVS] Bridge check: $line" | logger -t ovs-monitor
    done

# Set up systemd journal alerts
cat > /etc/systemd/system/network-alert.service << EOF
[Unit]
Description=Network Alert Service
After=network.target

[Service]
ExecStart=/usr/local/bin/network-alert.sh

[Install]
WantedBy=multi-user.target
EOF
```

### **9. Security Considerations**

```bash
# 1. Limit D-Bus access
# Edit /etc/dbus-1/system.d/org.freedesktop.network1.conf
# Restrict who can call networkd methods

# 2. Protect configuration files
chmod 600 /etc/systemd/network/*
chmod 600 /etc/network/interfaces

# 3. Use separate management network for OVS control
# Don't expose OVS management on public interfaces

# 4. Enable OVS security features
ovs-vsctl set bridge br0 fail-mode=secure
ovs-vsctl set-controller br0 ssl:...  # Use SSL for controller

# 5. Audit network changes
# Enable auditd for network configuration changes
auditctl -w /etc/systemd/network/ -p wa -k network-config
auditctl -w /etc/network/interfaces -p wa -k network-config
```

### **10. Testing Strategy**

Before deploying to production:

```bash
# 1. Test in VM or lab environment
# 2. Verify connectivity after each change
ping -c 4 8.8.8.8
ping -c 4 <gateway>

# 3. Test DNS resolution
nslookup google.com

# 4. Test routing
traceroute 8.8.8.8

# 5. Test performance
iperf3 -c <server>
mtr <destination>

# 6. Test failover (if using bonding/teaming)
# Disconnect one interface and verify connectivity

# 7. Test reboot persistence
reboot
# After reboot, verify all services and interfaces are up
```

---

## Common Scenarios

### **Scenario 1: Basic OVS Bridge with systemd-networkd**

**Goal:** Create an OVS bridge, enslave physical interface, assign IP to internal port.

```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
PHYSICAL_IF="eth0"
IP_ADDR="192.168.1.100/24"
GATEWAY="192.168.1.1"
DNS="8.8.8.8"

# 1. Create OVS bridge
ovs-vsctl add-br "$BRIDGE"

# 2. Add physical interface (no IP)
ovs-vsctl add-port "$BRIDGE" "$PHYSICAL_IF"

# 3. Create internal management port
ovs-vsctl add-port "$BRIDGE" "${BRIDGE}-int" -- \
    set interface "${BRIDGE}-int" type=internal

# 4. Configure physical interface (no IP, just link)
cat > /etc/systemd/network/10-${PHYSICAL_IF}.network << EOF
[Match]
Name=${PHYSICAL_IF}

[Network]
LinkLocalAddressing=no
DHCP=no

[Link]
RequiredForOnline=no
EOF

# 5. Configure internal port with IP
cat > /etc/systemd/network/20-${BRIDGE}-int.network << EOF
[Match]
Name=${BRIDGE}-int

[Network]
Address=${IP_ADDR}
Gateway=${GATEWAY}
DNS=${DNS}

[Link]
RequiredForOnline=yes
EOF

# 6. Restart networkd
systemctl restart systemd-networkd

# 7. Bring up interfaces
ip link set "$BRIDGE" up
ip link set "${BRIDGE}-int" up
ip link set "$PHYSICAL_IF" up

# 8. Verify
echo "=== Verification ==="
networkctl status "${BRIDGE}-int"
ovs-vsctl show
ping -c 4 "$GATEWAY" || echo "Warning: Cannot ping gateway"
ping -c 4 8.8.8.8 || echo "Warning: Cannot ping Internet"
```

### **Scenario 2: OVS VLAN Trunking**

**Goal:** Configure OVS port as trunk, create VLAN access ports.

```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
TRUNK_PORT="eth0"
VLANS=(10 20 30)

# 1. Create bridge if not exists
ovs-vsctl --may-exist add-br "$BRIDGE"

# 2. Add physical port as trunk
ovs-vsctl add-port "$BRIDGE" "$TRUNK_PORT"
ovs-vsctl set port "$TRUNK_PORT" trunks=$(IFS=,; echo "${VLANS[*]}")

# 3. Create VLAN access ports
for VLAN in "${VLANS[@]}"; do
    PORT_NAME="vlan${VLAN}"
    
    # Create internal port
    ovs-vsctl add-port "$BRIDGE" "$PORT_NAME" tag="$VLAN" -- \
        set interface "$PORT_NAME" type=internal
    
    # Configure with systemd-networkd
    cat > "/etc/systemd/network/30-${PORT_NAME}.network" << EOF
[Match]
Name=${PORT_NAME}

[Network]
Address=192.168.${VLAN}.1/24
EOF
    
    # Bring up
    ip link set "$PORT_NAME" up
done

# 4. Restart networkd
systemctl restart systemd-networkd

# 5. Verify
echo "=== VLAN Configuration ==="
ovs-vsctl show
ovs-ofctl dump-flows "$BRIDGE"
networkctl list
```

### **Scenario 3: OVS Bonding with LACP**

**Goal:** Create bonded interface for redundancy and load balancing.

```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
BOND_NAME="bond0"
INTERFACES=("eth0" "eth1")
IP_ADDR="192.168.1.100/24"
GATEWAY="192.168.1.1"

# 1. Create bridge
ovs-vsctl add-br "$BRIDGE"

# 2. Create bond with LACP
ovs-vsctl add-bond "$BRIDGE" "$BOND_NAME" "${INTERFACES[@]}" \
    bond_mode=balance-tcp \
    lacp=active \
    other_config:lacp-time=fast

# 3. Create internal port for management
ovs-vsctl add-port "$BRIDGE" "${BRIDGE}-int" -- \
    set interface "${BRIDGE}-int" type=internal

# 4. Configure internal port
cat > /etc/systemd/network/40-${BRIDGE}-int.network << EOF
[Match]
Name=${BRIDGE}-int

[Network]
Address=${IP_ADDR}
Gateway=${GATEWAY}
EOF

# 5. Configure physical interfaces (no IP)
for IFACE in "${INTERFACES[@]}"; do
    cat > "/etc/systemd/network/10-${IFACE}.network" << EOF
[Match]
Name=${IFACE}

[Network]
LinkLocalAddressing=no
DHCP=no
EOF
done

# 6. Restart networkd
systemctl restart systemd-networkd

# 7. Bring up interfaces
ip link set "$BRIDGE" up
ip link set "${BRIDGE}-int" up

# 8. Verify
echo "=== Bond Status ==="
ovs-appctl bond/show "$BOND_NAME"
ovs-appctl lacp/show "$BOND_NAME"
networkctl status "${BRIDGE}-int"
```

### **Scenario 4: Container Network with OVS**

**Goal:** Create dedicated OVS bridge for containers with port isolation.

```bash
#!/bin/bash
set -euo pipefail

BRIDGE="ovsbr1"
SUBNET="172.16.0.0/16"
BRIDGE_IP="172.16.0.1/16"

# 1. Create dedicated container bridge
ovs-vsctl add-br "$BRIDGE"

# 2. Create internal port for bridge IP
ovs-vsctl add-port "$BRIDGE" "${BRIDGE}-int" -- \
    set interface "${BRIDGE}-int" type=internal

# 3. Configure internal port
cat > /etc/systemd/network/50-${BRIDGE}-int.network << EOF
[Match]
Name=${BRIDGE}-int

[Network]
Address=${BRIDGE_IP}
EOF

systemctl restart systemd-networkd

# 4. Bring up
ip link set "$BRIDGE" up
ip link set "${BRIDGE}-int" up

# 5. Enable IP forwarding
sysctl -w net.ipv4.ip_forward=1
echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf

# 6. Set up NAT for containers
UPLINK_IF="eth0"  # Change to your uplink interface
iptables -t nat -A POSTROUTING -s "$SUBNET" -o "$UPLINK_IF" -j MASQUERADE

# 7. Function to attach container interface
attach_container() {
    local CONTAINER_IF=$1
    local CONTAINER_NAME=$2
    
    # Add port to bridge
    ovs-vsctl add-port "$BRIDGE" "$CONTAINER_IF"
    
    # Optional: Set port isolation (prevents container-to-container traffic)
    # ovs-vsctl set port "$CONTAINER_IF" other_config:port-security=true
    
    echo "Attached $CONTAINER_IF ($CONTAINER_NAME) to $BRIDGE"
}

# 8. Example: Attach a container
# attach_container veth123 my-container

# 9. Verify
echo "=== Container Bridge Status ==="
ovs-vsctl show
ip addr show "${BRIDGE}-int"
```

### **Scenario 5: High Availability with OVS and Multiple Uplinks**

**Goal:** Create redundant connectivity with automatic failover.

```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
UPLINKS=("eth0" "eth1")
IP_ADDR="192.168.1.100/24"
GATEWAY="192.168.1.1"

# 1. Create bridge
ovs-vsctl add-br "$BRIDGE"

# 2. Add uplinks as separate ports (not bonded)
for UPLINK in "${UPLINKS[@]}"; do
    ovs-vsctl add-port "$BRIDGE" "$UPLINK"
done

# 3. Enable STP for loop prevention
ovs-vsctl set bridge "$BRIDGE" stp_enable=true

# 4. Create internal port
ovs-vsctl add-port "$BRIDGE" "${BRIDGE}-int" -- \
    set interface "${BRIDGE}-int" type=internal

# 5. Configure internal port
cat > /etc/systemd/network/60-${BRIDGE}-int.network << EOF
[Match]
Name=${BRIDGE}-int

[Network]
Address=${IP_ADDR}
Gateway=${GATEWAY}
DNS=8.8.8.8

[Route]
Gateway=${GATEWAY}
Metric=100

[Link]
RequiredForOnline=yes
EOF

# 6. Restart networkd
systemctl restart systemd-networkd

# 7. Bring up
ip link set "$BRIDGE" up
ip link set "${BRIDGE}-int" up

# 8. Verify STP
echo "=== STP Status ==="
ovs-appctl stp/show "$BRIDGE"

# 9. Test failover
echo "=== Testing Failover ==="
echo "Disconnect one uplink and verify connectivity..."
```

### **Scenario 6: Migration from Linux Bridge to OVS**

**Goal:** Migrate existing Linux bridge to OVS without downtime.

```bash
#!/bin/bash
set -euo pipefail

OLD_BRIDGE="br0"
NEW_BRIDGE="ovsbr0"
PHYSICAL_IF="eth0"

# 1. Capture current configuration
CURRENT_IP=$(ip -4 addr show "$OLD_BRIDGE" | grep inet | awk '{print $2}')
CURRENT_GW=$(ip route | grep default | awk '{print $3}')

echo "Current IP: $CURRENT_IP"
echo "Current Gateway: $CURRENT_GW"

# 2. Create new OVS bridge
ovs-vsctl add-br "$NEW_BRIDGE"
ovs-vsctl add-port "$NEW_BRIDGE" "${NEW_BRIDGE}-int" -- \
    set interface "${NEW_BRIDGE}-int" type=internal

# 3. Configure new bridge (don't activate yet)
cat > /etc/systemd/network/70-${NEW_BRIDGE}-int.network << EOF
[Match]
Name=${NEW_BRIDGE}-int

[Network]
Address=${CURRENT_IP}
Gateway=${CURRENT_GW}
DNS=8.8.8.8
EOF

# 4. Bring up new bridge (no IP conflicts yet)
ip link set "$NEW_BRIDGE" up
ip link set "${NEW_BRIDGE}-int" up

# 5. Remove physical interface from old bridge
ip link set "$PHYSICAL_IF" nomaster

# 6. Add physical interface to new OVS bridge
ovs-vsctl add-port "$NEW_BRIDGE" "$PHYSICAL_IF"

# 7. Activate networkd configuration
systemctl restart systemd-networkd

# 8. Remove old bridge
ip link set "$OLD_BRIDGE" down
brctl delbr "$OLD_BRIDGE"

# 9. Update /etc/network/interfaces if needed
sed -i "s/$OLD_BRIDGE/$NEW_BRIDGE/g" /etc/network/interfaces

# 10. Verify
echo "=== Migration Complete ==="
ovs-vsctl show
networkctl status "${NEW_BRIDGE}-int"
ping -c 4 "$CURRENT_GW"
```

### **Scenario 7: OVS with VXLAN Overlay Network**

**Goal:** Create VXLAN tunnel between two OVS bridges for L2 extension.

**Node 1:**
```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
VXLAN_PORT="vxlan1"
REMOTE_IP="203.0.113.2"  # Remote node IP
VNI="1000"               # VXLAN Network Identifier

# 1. Create bridge
ovs-vsctl add-br "$BRIDGE"

# 2. Create VXLAN tunnel
ovs-vsctl add-port "$BRIDGE" "$VXLAN_PORT" -- \
    set interface "$VXLAN_PORT" type=vxlan \
    options:remote_ip="$REMOTE_IP" \
    options:key="$VNI"

# 3. Verify
ovs-vsctl show
ovs-vsctl list interface "$VXLAN_PORT"
```

**Node 2:**
```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
VXLAN_PORT="vxlan1"
REMOTE_IP="203.0.113.1"  # First node IP
VNI="1000"

# Same configuration as Node 1, but with different REMOTE_IP
ovs-vsctl add-br "$BRIDGE"
ovs-vsctl add-port "$BRIDGE" "$VXLAN_PORT" -- \
    set interface "$VXLAN_PORT" type=vxlan \
    options:remote_ip="$REMOTE_IP" \
    options:key="$VNI"

ovs-vsctl show
```

### **Scenario 8: OVS with QoS and Rate Limiting**

**Goal:** Implement bandwidth limits and QoS for different traffic types.

```bash
#!/bin/bash
set -euo pipefail

BRIDGE="br0"
PORT="vport1"
MAX_RATE="1000000000"  # 1 Gbps in bps
GUARANTEED_RATE="100000000"  # 100 Mbps in bps

# 1. Create QoS policy
QOS_UUID=$(ovs-vsctl create qos type=linux-htb \
    other-config:max-rate="$MAX_RATE")

# 2. Create queue with guaranteed bandwidth
QUEUE_UUID=$(ovs-vsctl create queue \
    other-config:min-rate="$GUARANTEED_RATE" \
    other-config:max-rate="$MAX_RATE")

# 3. Attach queue to QoS
ovs-vsctl set qos "$QOS_UUID" queues=0="$QUEUE_UUID"

# 4. Apply QoS to port
ovs-vsctl set port "$PORT" qos="$QOS_UUID"

# 5. Set ingress policing (rate limiting)
ovs-vsctl set interface "$PORT" ingress_policing_rate=1000  # Kbps
ovs-vsctl set interface "$PORT" ingress_policing_burst=100   # Kb

# 6. Verify
echo "=== QoS Configuration ==="
ovs-vsctl list qos
ovs-vsctl list queue
ovs-vsctl list port "$PORT"
ovs-vsctl list interface "$PORT"
```

---

## Troubleshooting Guide

### **Problem: Network connectivity lost after configuration change**

**Diagnosis:**
```bash
# Check interface status
ip link show
ip addr show

# Check routing
ip route show

# Check DNS
cat /etc/resolv.conf
resolvectl status

# Check which service is running
systemctl is-active NetworkManager systemd-networkd networking
```

**Solution:**
```bash
# If using systemd-networkd
systemctl restart systemd-networkd
networkctl reconfigure <interface>

# If using ifupdown
ifdown <interface> && ifup <interface>

# If using NetworkManager
nmcli connection down <connection>
nmcli connection up <connection>

# Check logs
journalctl -u systemd-networkd -n 50
journalctl -u NetworkManager -n 50
```

### **Problem: OVS bridge not forwarding traffic**

**Diagnosis:**
```bash
# Check bridge status
ovs-vsctl show
ovs-ofctl show br0

# Check flows
ovs-ofctl dump-flows br0

# Check datapath
ovs-dpctl show

# Check interface states
ip link show | grep ovs
```

**Solution:**
```bash
# Add default forwarding rule
ovs-ofctl add-flow br0 "priority=0,actions=NORMAL"

# Check if interfaces are UP
ip link set br0 up
ip link set <port> up

# Verify STP not blocking
ovs-appctl stp/show br0

# Check logs
journalctl -u openvswitch-switch -n 50
```

### **Problem: Multiple network managers conflicting**

**Diagnosis:**
```bash
# Check running services
systemctl is-active NetworkManager systemd-networkd networking

# Check which manages interface
nmcli device show <interface>
networkctl status <interface>
ifquery <interface>
```

**Solution:**
```bash
# Choose ONE and disable others
systemctl stop NetworkManager
systemctl disable NetworkManager
systemctl mask NetworkManager

# Or disable systemd-networkd
systemctl stop systemd-networkd
systemctl disable systemd-networkd
systemctl mask systemd-networkd
```

### **Problem: D-Bus communication errors**

**Diagnosis:**
```bash
# Check D-Bus daemon
systemctl status dbus

# Check permissions
ls -la /etc/dbus-1/system.d/

# Test D-Bus connection
busctl list | grep network
```

**Solution:**
```bash
# Restart D-Bus (careful!)
systemctl restart dbus

# Check policy files
cat /etc/dbus-1/system.d/org.freedesktop.network1.conf

# Check logs
journalctl -u dbus -n 50
```

---

## Performance Tuning

### **OVS Performance Optimization**

```bash
# 1. Enable DPDK (Data Plane Development Kit)
ovs-vsctl set Open_vSwitch . other_config:dpdk-init=true

# 2. Set datapath type to DPDK
ovs-vsctl set bridge br0 datapath_type=netdev

# 3. Increase OVS memory
ovs-vsctl set Open_vSwitch . other_config:dpdk-socket-mem=1024

# 4. Enable jumbo frames
ovs-vsctl set interface eth0 mtu_request=9000

# 5. Optimize flow cache
ovs-vsctl set Open_vSwitch . other_config:flow-restore-wait=true

# 6. Monitor performance
ovs-appctl dpif/show-dp-features
ovs-appctl dpctl/show -s
```

### **systemd-networkd Performance**

```bash
# 1. Optimize DHCP
# In .network file:
[DHCP]
UseMTU=true
RouteMetric=100
ClientIdentifier=mac

# 2. Disable IPv6 if not needed
[Network]
LinkLocalAddressing=ipv4

# 3. Reduce log verbosity
systemctl edit systemd-networkd
[Service]
Environment=SYSTEMD_LOG_LEVEL=warning
```

---

## Security Hardening

### **Restrict D-Bus Access**

```bash
# Edit /etc/dbus-1/system.d/org.freedesktop.network1.conf
cat > /etc/dbus-1/system.d/org.freedesktop.network1.conf << EOF
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <policy user="root">
    <allow own="org.freedesktop.network1"/>
    <allow send_destination="org.freedesktop.network1"/>
  </policy>
  <policy group="netdev">
    <allow send_destination="org.freedesktop.network1"/>
  </policy>
  <policy context="default">
    <deny send_destination="org.freedesktop.network1"/>
  </policy>
</busconfig>
EOF
```

### **OVS Security**

```bash
# 1. Enable port security
ovs-vsctl set port eth0 other_config:port-security=true

# 2. Set fail-secure mode
ovs-vsctl set bridge br0 fail-mode=secure

# 3. Use SSL for controller
ovs-vsctl set-ssl /etc/ovs-privkey.pem \
    /etc/ovs-cert.pem \
    /etc/ovs-cacert.pem

# 4. Limit MAC learning
ovs-vsctl set bridge br0 other-config:mac-table-size=2048

# 5. Enable RST P
ovs-vsctl set bridge br0 rstp_enable=true
```

---

## Conclusion

This guide covers the comprehensive interaction between D-Bus, systemd-networkd, Open vSwitch, and /etc/network/interfaces. Key takeaways:

1. **Choose one network manager** and stick with it
2. **Use D-Bus** for service communication and monitoring
3. **OVS provides powerful networking** but requires proper integration
4. **Test changes** in non-production environments first
5. **Always backup** before making changes
6. **Document everything** for future reference

For the nm-monitor project specifically, the system uses:
- **D-Bus** for RPC and introspection
- **OVS** for virtual networking
- **NetworkManager** for atomic operations
- **Blockchain ledger** for accountability

This ensures zero-connectivity-loss deployments with complete audit trails.
