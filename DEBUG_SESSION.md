# Debug Session State - 2025-10-15

## Current Status
Testing installation script fix on VPS: root@80.209.240.244

## Problem History

### Issue 1: Bridge Already Exists ✅ FIXED
- **Symptom**: `add-br` failed because bridge existed
- **Fix**: Added `--may-exist` flags to bridge/port operations
- **Commit**: aa50baa

### Issue 2: Port Deleted After Creation ✅ FIXED  
- **Symptom**: OVS logs showed port added then deleted 9 seconds later
- **Root Cause**: systemd-networkd reload removed port because config didn't exist yet
- **Evidence**: `/var/log/openvswitch/ovs-vswitchd.log`
  ```
  02:36:33 - added interface ens1 on port 1
  02:36:42 - deleted interface ens1 on port 1
  ```
- **Fix**: Reordered operations in install script
  1. Create bridge
  2. Create networkd configs ← MOVED UP
  3. Reload networkd ← MOVED UP
  4. Add port ← Now networkd won't remove it
  5. Wait for IP
  6. Test connectivity
- **Commit**: c5a1538

## Next Steps
1. Pull latest code on VPS: `cd /git/nm-monitor && git pull`
2. Run install script: `sudo ./scripts/install-compliant.sh`
3. Monitor for success or new issues

## Key Files
- `/git/nm-monitor/scripts/install-compliant.sh` - Installation script
- `/git/nm-monitor/src/bin/ovsdb-dbus-wrapper.rs` - D-Bus wrapper with --may-exist
- `/var/log/openvswitch/ovs-vswitchd.log` - OVS operation logs (critical for debugging)

## VPS Details
- Host: 80.209.240.244
- SSH Key: /home/jeremy/.ssh/gbjh2
- Uplink: ens1
- Bridge: ovsbr0
- Current IP: 80.209.240.244/22
- Gateway: 80.209.240.1
