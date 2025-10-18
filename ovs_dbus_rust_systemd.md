## Wrapping Open vSwitch with D-Bus for Orchestration

### Background and Motivation

Open vSwitch (OVS) is managed via CLI tools and a proprietary OVSDB protocol. It lacks native D-Bus integration, which limits orchestration with tools like systemd-networkd. Wrapping OVS in a D-Bus interface facilitates standard IPC integration, allowing higher-level orchestration and system coordination.

### Existing D-Bus Interfaces for OVS and OVN

- **OVS and OVN:** No native D-Bus support. OVS uses OVSDB (RFC 7047). OVN uses northbound/southbound DBs.
- **NetworkManager:** Offers limited OVS support via internal OVSDB manipulation. Minimal D-Bus exposure.
- **OF-CONFIG (OpenFlow):** Implements NETCONF-to-OVS bridge using D-Bus internally between server and agents.

### Projects Wrapping OVS over D-Bus

- **openQA (os-autoinst):** Implements `org.opensuse.os_autoinst.switch` with methods like `SetVlan`, `Show`. Internally uses `ovs-vsctl` and `ovs-ofctl`.
- **Wicked (openSUSE):** Same implementation as openQA, packaged as a systemd service.
- **OVN:** No known direct D-Bus wrappers. Managed through OVN databases using `ovn-nbctl`, `ovn-sbctl`.

### Writing a D-Bus Service in Rust

- **Interfacing with OVS:**
  1. Shell out to OVS CLI tools.
  2. Use OVSDB natively via the `ovsdb` Rust crate (async, Serde, Tokio).
- **D-Bus Crates in Rust:**
  - `dbus-rs`: Synchronous and async API.
  - `zbus`: Async-native, ergonomic, recommended.

```rust
#[zbus::dbus_interface(name = "org.mycompany.ovs")]
impl OvsController {
    fn add_bridge(&self, name: &str) -> Result<(), zbus::Error> {
        // Call ovs-vsctl or use OVSDB crate
    }
}
```

- **Security:** D-Bus policy files in `/etc/dbus-1/system.d/` to restrict access.

### Systemd Integration

- **Service Unit (Type=dbus):**

```ini
[Service]
Type=dbus
BusName=org.mycompany.ovs
ExecStart=/usr/local/bin/ovs-dbus-service
```

- **D-Bus Activation:** File `/usr/share/dbus-1/system-services/org.mycompany.ovs.service`:

```ini
[D-BUS Service]
Name=org.mycompany.ovs
Exec=/usr/local/bin/ovs-dbus-service
SystemdService=ovs-dbus.service
```

- **Ordering:** `After=ovsdb-server.service ovs-vswitchd.service` if needed.

### Deployment with systemd-networkd

- **Static Setup:** Create OVS bridges/ports before networkd applies `.network` files.
- **Dynamic Coordination:** Let networkd configure IP/DHCP; use D-Bus service to orchestrate switching logic.
- **Use Case:** On VM start, use D-Bus method `AddBridge("br-ens160")`, then networkd auto-applies config to `br-ens160`.

### Real-World Example

- **openQA:**
  - D-Bus methods wrap `ovs-vsctl`.
  - Uses systemd for service lifecycle.
  - Waits for IP assignment before proceeding.

### Conclusion

No native D-Bus interface for OVS/OVN exists. Rust can be used to implement a robust wrapper exposing high-level D-Bus methods. Systemd integration via `Type=dbus` and D-Bus activation allows clean boot-time orchestration. Combined with systemd-networkd, this forms a modern, policy-driven SDN approach on Linux hosts.

