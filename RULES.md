LLM RULES: NetworkManager + OVS Bridge (D-Bus Introspection Version)
0) Scope

All LLM actions involving Open vSwitch (OVS) bridges must be done through NetworkManager using D-Bus.
No direct shell commands (ovs-vsctl, ip, nmcli) are allowed for configuration — only for verification.

1) Canonical Source

The only source of truth is /home/claude/docs/networkmanager.dev/.

No assumptions outside the documentation.

D-Bus introspection replaces parsing CLI output.

2) Hard Prohibitions

Never use ovs-vsctl to create, modify, or delete bridges, ports, or interfaces.

Never use ip addr or nmcli dev show for configuration discovery.

Never use master/slave; use controller.

Never assign IPs to physical NICs when enslaved. IP must go on the ovs-interface.

3) Correct Hierarchy

ovs-bridge (controller)
├─ ovs-port (controller = bridge)
│ └─ ovs-interface (controller = port) ← IP/DNS/Gateway here
└─ ovs-port (controller = bridge)
└─ ethernet (controller = port) ← enslaved NIC, no IP

4) Creation Sequence

Create each layer with AddConnection2() using proper settings:

connection.type = ovs-bridge

connection.interface-name = bridge0

ipv4.method = disabled for the bridge

Add ovs-port and ovs-interface connections linked via controller

Add physical ethernet connection linked to an ovs-port

Activate only the bridge using ActivateConnection().

5) IP Placement

Extract current IP info from D-Bus, not shell commands.
Use the following interfaces:

org.freedesktop.NetworkManager.Device.Ip4Config

org.freedesktop.NetworkManager.IP4Config.Addresses

org.freedesktop.NetworkManager.IP4Config.Gateway

org.freedesktop.NetworkManager.IP4Config.Nameservers

Apply the extracted IP, gateway, and DNS to the ovs-interface.

6) D-Bus Introspection Method

To inspect live data:

busctl get-property org.freedesktop.NetworkManager /org/freedesktop/NetworkManager/Devices/N org.freedesktop.NetworkManager.Device Ip4Config
busctl introspect org.freedesktop.NetworkManager /org/freedesktop/NetworkManager/IP4Config/M

The same can be done with godbus in Go code for automated retrieval.

7) Atomic Activation

Create all connections first, then activate only the bridge.
NetworkManager will bring up all child connections automatically.
Do not manually bring up ports or interfaces.

8) Verification

Confirm via D-Bus:

Device state = ACTIVATED

IP4Config has correct address

Active connection matches ovs-interface

Optional read-only verification:

nmcli -t -f NAME,DEVICE,STATE con show

ip addr show dev <bridge>

9) Checkpoint and Rollback

Always create a checkpoint before changes:
nmcli general checkpoint create
If something fails:
nmcli general checkpoint rollback

10) Logging

All scripts must log to /var/log/nm-ovs-setup.log.
Use tee to log stdout and stderr.
Always record pre and post state with:

nmstatectl show

busctl tree org.freedesktop.NetworkManager

11) Unmanaged Devices

If unmanaged OVS devices exist, they must be either:

Recreated under NetworkManager control, or

Deleted and rebuilt correctly.

Final state must have no unmanaged OVS objects.

12) Compliance Checklist

IP only on ovs-interface

Physical NIC enslaved, no IP

Bridge active

Introspection shows correct IP4Config

No use of ovs-vsctl

Activation is atomic and reversible via checkpoint
