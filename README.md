# Proxmox OVS Container Auto-Attach Plan

This runbook specifies everything needed to implement an automation suite that guarantees every LXC container interface on a Proxmox host is bridged to `ovsbr0`. It combines a Go daemon that listens to netlink events with an `LD_PRELOAD` helper library that captures interfaces created inside container namespaces. No source code is provided here; the goal is a complete blueprint so a developer can start coding immediately.

The host environment is Proxmox VE 8+ running on a Btrfs root (e.g. `/@`, `/@root`, `/@opt`). All commands assume root privileges unless noted.

---

## 1. Project Overview

- **Objective:** Keep LXC veth/tap interfaces attached to Open vSwitch bridge `ovsbr0`, keep `/etc/nmstate/nmstate-dynamic.yaml` and `/etc/network/interfaces` synchronized, and have the setup survive reboots. Monitoring must detect interface creation/removal even when netlink events are delayed or obscured by namespaces.
- **Approach:**
  - *Netlink daemon (Go):* Fast, kernel-native detection of link changes. Ideal for real-time updates when interfaces are created/destroyed on the host.
  - *`LD_PRELOAD` helper (C):* Intercepts libc networking calls made from inside containers or helper binaries, sending deterministic notifications to the daemon when veth/tap devices are created even if the host misses netlink events.
- **Constraints:**
  - Must run cleanly under systemd on Proxmox.
  - File writes must be atomic and idempotent.
  - Debounce rapid link events (~500 ms) to avoid repeated file rewrites.
  - Keep implementation compatible with existing Btrfs subvolume layout (e.g. `/@`, `/@root`, `/@opt`, `/@snapshots`).
  - Logging goes to journald (systemd unit controlled) and should clearly indicate detected interfaces and refresh results.

---

## 2. Go Daemon Specification (`nmstate-monitor`)

### 2.1 Module Setup
- **Module path:** `nmstate-monitor`
- **Go version:** `go 1.21`
- **Dependencies:**
  - `github.com/vishvananda/netlink`
  - `gopkg.in/yaml.v3`
  - Standard library only for logging (`log`), signals, file IO, etc.
- **Directory layout:**
  ```text
  go/src/nmstate-monitor/
    go.mod
    go.sum
    main.go
  ```

### 2.2 Netlink Watch Loop
- `main()` pseudocode:
  1. `logger := log.New(os.Stdout, "nmstate-monitor: ", log.LstdFlags|log.Lmicroseconds)`.
  2. Call `refreshState(logger)` once; log errors but continue.
  3. `updates := make(chan netlink.LinkUpdate)` and `done := make(chan struct{})`.
  4. `netlink.LinkSubscribe(updates, done)` for link events.
  5. Capture `SIGINT` and `SIGTERM` via `signal.Notify` to shut down gracefully.
  6. Debounce logic:
     ```go
     var timer *time.Timer
     var timerCh <-chan time.Time
     const debounce = 500 * time.Millisecond
     ```
     - On each relevant event, if `timer != nil`, stop it (drain channel if needed). Then `timer = time.NewTimer(debounce)` and set `timerCh = timer.C`.
     - On `<-timerCh`, stop/reset timer and invoke `refreshState(logger)`.
  7. Relevant events are those whose `attrs.Name` starts with `"veth"` or `"tap"`.
  8. Log every accepted event: `logger.Printf("netlink event for %s", name)`.

### 2.3 State Refresh Function
- `refreshState(logger)` steps:
  1. `ports, err := discoverManagedPorts()`
     - `links, err := netlink.LinkList()`
     - Filter names with `strings.HasPrefix(name, "veth")` or `strings.HasPrefix(name, "tap")`.
     - Use a `map[string]struct{}` to dedupe, then convert to slice and `sort.Strings`.
  2. `writeNMState(ports)`
     - Ensure `/etc/nmstate` exists (`os.MkdirAll` 0755).
     - Compose YAML structure:
       ```yaml
       interfaces:
         - name: ovsbr0
           type: ovs-bridge
           state: up
           bridge:
             port:
               - name: vethABCDEFG
               - name: tapXYZ
         - name: vethABCDEFG
           type: ovs-port
           state: up
           controller: ovsbr0
         - name: tapXYZ
           type: ovs-port
           state: up
           controller: ovsbr0
       ```
       If `len(ports) == 0`, omit `bridge.port` entirely but still declare `ovsbr0`.
     - Serialize using `yaml.Marshal`. Use helper `writeFileIfChanged(path, data, 0644)` to avoid unnecessary writes.
  3. `updateInterfacesFile(ports)`
     - Desired block skeleton:
       ```text
       # BEGIN nmstate-monitor
       # Managed by nmstate-monitor, do not edit manually.
       auto vethXXX
       allow-ovs vethXXX
       iface vethXXX inet manual
           ovs_type OVSPort
           ovs_bridge ovsbr0

       auto tapYYY
       allow-ovs tapYYY
       iface tapYYY inet manual
           ovs_type OVSPort
           ovs_bridge ovsbr0
       # END nmstate-monitor
       ```
       - When `len(ports) == 0` replace the per-port block with `# No container OVS ports detected.`
     - Use `applyManagedBlock(existing, newBlock)` to replace or append the block bounded by the marker comments.
     - Write atomically with `writeFileIfChanged`.
  4. Log `logger.Printf("state updated, managed ports: %v", ports)`.

### 2.4 File Handling Helpers
- `writeFileIfChanged(path, data, perm)`:
  - `os.ReadFile` existing. If identical to `data`, return.
  - Else call `writeFileAtomic`.
- `writeFileAtomic(path, data, perm)`:
  1. Create temp in same directory (`os.CreateTemp`).
  2. Write data, `tmp.Chmod(perm)`, `tmp.Sync()`, `tmp.Close()`.
  3. `os.Rename(tmp.Name(), path)`.
- `applyManagedBlock(content, block)` logic:
  - `strings.Index` for start marker.
  - If not found, append block (ensure file ends with newline).
  - If found, locate end marker and replace segment.

### 2.5 Logging Expectations
- On start: `nmstate-monitor: YYYY/MM/DD HH:MM:SS state updated, managed ports: [...]` followed by `listening for interface updates`.
- On netlink event: `netlink event for vethXYZ`.
- On refresh failure: `state refresh failed: ...` (do not exit).
- All logs go to stdout; systemd captures them in journald.

### 2.6 Systemd Unit
- File: `/etc/systemd/system/nmstate-monitor.service`
  ```ini
  [Unit]
  Description=Monitor LXC interfaces and sync nmstate / interfaces configuration
  After=network-online.target
  Wants=network-online.target

  [Service]
  Type=simple
  ExecStart=/usr/local/bin/nmstate-monitor
  Restart=on-failure
  RestartSec=2s
  StandardOutput=journal
  StandardError=journal

  [Install]
  WantedBy=multi-user.target
  ```
- Install commands:
  ```bash
  cd /home/jeremy/go/src/nmstate-monitor
  go mod tidy
  go build -o nmstate-monitor
  sudo install -m 0755 nmstate-monitor /usr/local/bin/
  sudo install -m 0644 nmstate-monitor.service /etc/systemd/system/
  sudo systemctl daemon-reload
  sudo systemctl enable --now nmstate-monitor
  sudo systemctl status nmstate-monitor
  ```

---

## 3. `LD_PRELOAD` Helper Library

### 3.1 Rationale
- Some network operations originate inside privileged container utilities or inside the guest namespace. The host may miss the initial netlink notifications.
- Intercepting libc calls (`socket`, `ioctl`, `if_nametoindex`, `if_indextoname`, `netlink` system calls) allows the helper to capture the interface name at creation time and immediately notify the daemon.
- This is especially important for short-lived veth pairs that might be destroyed before the host listener reacts.

### 3.2 Communication Method
- Use a Unix datagram socket (e.g., `/run/nmstate-monitor.sock`). Advantages:
  - No FIFO cleanup race.
  - Supports small messages like `{"iface":"vethXYZ","pid":1234}`.
- Protocol:
  - On preload init (`constructor`), read `NMSTATE_MONITOR_SOCKET` env var (default `/run/nmstate-monitor.sock`).
  - When a hook detects a new interface (e.g., `if_nametoindex` sees unknown `veth` or `tap`), send one datagram: `iface=vethXYZ;pid=PID;event=create`.
  - Optionally send `event=delete` when interfaces are closed (if detection is feasible).

### 3.3 Implementation Outline
- Directory: `/home/jeremy/go/src/ldpreload-interfaces/` (re-use existing structure).
- Source file `nmstate_preload.c` containing hooks.
- Key functions:
  - `static int notify_daemon(const char *iface)` to open the socket and send a message.
  - Override `if_nametoindex` and `if_indextoname`. After calling the real function (via `dlsym(RTLD_NEXT, ...)`), if the name matches `veth*`/`tap*`, call `notify_daemon`.
  - Optionally hook `socket(AF_NETLINK, ...)` to ensure the library sees netlink operations as well.
- Logging inside the `.so` should go to stderr only when `NMSTATE_MONITOR_LOG_LEVEL=debug`.

### 3.4 Build System
- `Makefile` sample targets:
  ```make
  CC=gcc
  CFLAGS=-fPIC -Wall -Wextra -O2
  LDFLAGS=-shared -ldl
  TARGET=libnmstate_preload.so

  all: $(TARGET)

  $(TARGET): nmstate_preload.o
      $(CC) $(LDFLAGS) -o $@ $^

  nmstate_preload.o: nmstate_preload.c
      $(CC) $(CFLAGS) -c $< -o $@

  install: $(TARGET)
      install -m 0755 $(TARGET) /usr/local/lib/
      install -d /etc/ld.so.preload.d
      printf '/usr/local/lib/$(TARGET)\n' > /etc/ld.so.preload.d/nmstate-monitor.conf

  clean:
      rm -f $(TARGET) nmstate_preload.o
  ```
- If global preload is not desired, skip writing `nmstate-monitor.conf` and instruct admins to configure per-container.

### 3.5 Runtime Configuration
- Prefer per-container activation via LXC config:
  ```ini
  lxc.environment = LD_PRELOAD=/usr/local/lib/libnmstate_preload.so
  lxc.environment = NMSTATE_MONITOR_SOCKET=/run/nmstate-monitor.sock
  lxc.environment = NMSTATE_MONITOR_LOG_LEVEL=info
  ```
- For system-wide tests, temporarily add the `.so` to `/etc/ld.so.preload.d/nmstate-monitor.conf`; document how to remove it to avoid interfering with unrelated processes.

### 3.6 Go Daemon Integration
- Add a goroutine to listen on the Unix datagram socket:
  - `net.ListenUnixgram("unixgram", &net.UnixAddr{Name: "/run/nmstate-monitor.sock", Net: "unixgram"})`.
  - Ensure socket directory exists (`/run/nmstate-monitor` 0755).
  - On each message, parse interface name and push onto the same channel that netlink events use (e.g., `events <- iface`).
- Deduping strategy:
  - Maintain a `map[string]time.Time` of last notification.
  - When netlink refresh runs, the full re-scan ensures actual state; preload notifications just accelerate detection.

---

## 4. Test and Validation Plan

### 4.1 Environment Checks
- Confirm OVS bridge exists:
  ```bash
  sudo ovs-vsctl show
  sudo ip -o link show ovsbr0
  ```
- Capture baseline nmstate state for reference:
  ```bash
  sudo /root/.cargo/bin/nmstatectl show > /root/nmstate-base.yaml
  ```

### 4.2 Build & Install Sequence
1. **Go daemon**
   ```bash
   cd /home/jeremy/go/src/nmstate-monitor
   go mod tidy
   go build -o nmstate-monitor
   sudo install -m 0755 nmstate-monitor /usr/local/bin/
   sudo install -m 0644 nmstate-monitor.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now nmstate-monitor
   sudo journalctl -u nmstate-monitor -n 20
   ```
2. **LD_PRELOAD library**
   ```bash
   cd /home/jeremy/go/src/ldpreload-interfaces
   make
   sudo make install
   # optional: configure per-container or global preload
   ```

### 4.3 Functional Testing
- Start daemon logs: `sudo journalctl -u nmstate-monitor -f` (expect “listening for interface updates”).
- Create test LXC container:
  ```bash
  sudo lxc-create -t download -n nmstate-test -- --dist debian --release bookworm --arch amd64
  sudo perl -pi -e 's/lxcbr0/ovsbr0/' /var/lib/lxc/nmstate-test/config
  sudo lxc-start -n nmstate-test
  ```
- Verify host sees new veth:
  ```bash
  ip -o link | grep veth
  ```
- Check daemon logs for netlink/preload events.
- Inspect outputs:
  ```bash
  sudo cat /etc/nmstate/nmstate-dynamic.yaml
  sudo tail -n 40 /etc/network/interfaces
  ```
- Stop container and confirm cleanup:
  ```bash
  sudo lxc-stop -n nmstate-test
  sudo lxc-destroy -n nmstate-test
  sudo cat /etc/nmstate/nmstate-dynamic.yaml
  sudo tail -n 40 /etc/network/interfaces
  ```
- If preload is enabled, ensure messages are received even when netlink events are suppressed (simulate by temporarily disabling netlink subscription and verifying notifications still arrive).

### 4.4 Post-Test Cleanup
- Remove temporary LXC container, revert preload configuration if applied system-wide, and archive log outputs if needed for documentation.

---

## 5. Deployment & Maintenance

- **Boot persistence:** systemd unit ensures daemon restarts automatically; the Unix socket should be recreated by the daemon on start.
- **Config backups:** capture `/etc/nmstate/nmstate-dynamic.yaml` and `/etc/network/interfaces` before enabling automation. Keep `nmstate-base.yaml` for reference.
- **Manual refresh:** run `sudo systemctl restart nmstate-monitor` to force a rescan; monitor via `journalctl`.
- **Firmware caveats:** AMD CPPC/SRSO BIOS warnings are unrelated to this automation. Document them separately; no changes to the automation are needed.
- **Btrfs snapshots:** use read-only snapshots for safe testing/backups:
  ```bash
  sudo btrfs subvolume snapshot -r /@ /mnt/btrfs-snapshots/@-clone-$(date +%F)
  sudo btrfs subvolume snapshot -r /@root /mnt/btrfs-snapshots/@root-$(date +%F)
  ```

---

## 6. Appendices

### 6.1 Sample `/etc/nmstate/nmstate-dynamic.yaml`
- **No ports:**
  ```yaml
  interfaces:
    - name: ovsbr0
      type: ovs-bridge
      state: up
  ```
- **With ports:**
  ```yaml
  interfaces:
    - name: ovsbr0
      type: ovs-bridge
      state: up
      bridge:
        port:
          - name: veth1234
          - name: tap5678
    - name: veth1234
      type: ovs-port
      state: up
      controller: ovsbr0
    - name: tap5678
      type: ovs-port
      state: up
      controller: ovsbr0
  ```

### 6.2 Managed Block Example (`/etc/network/interfaces`)
```
# BEGIN nmstate-monitor
# Managed by nmstate-monitor, do not edit manually.
auto veth1234
allow-ovs veth1234
iface veth1234 inet manual
    ovs_type OVSPort
    ovs_bridge ovsbr0

auto tap5678
allow-ovs tap5678
iface tap5678 inet manual
    ovs_type OVSPort
    ovs_bridge ovsbr0
# END nmstate-monitor
```

### 6.3 Systemd Commands Reference
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now nmstate-monitor
sudo systemctl status nmstate-monitor
sudo journalctl -u nmstate-monitor -f
```

---

With this blueprint, a developer can write the Go daemon, the preload library, unit tests, and deployment scripts without ambiguity. Adjust paths and hostnames (`castlebox`, `/home/jeremy`) as needed for other environments.
