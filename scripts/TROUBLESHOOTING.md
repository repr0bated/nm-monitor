# Troubleshooting Guide for nm-monitor Scripts

## Overview
This guide helps troubleshoot common issues with the nm-monitor installation and network introspection scripts.

## Scripts Covered
1. `introspect-network.sh` - Auto-detects network configuration
2. `install-with-network-plugin.sh` - Installs ovs-port-agent with network setup

## Common Issues and Solutions

### 1. Sudo/Permission Issues

#### introspect-network.sh
- **Issue**: "No primary interface detected" or limited network information
- **Cause**: Some network information requires root access
- **Solution**: Run with sudo: `sudo ./scripts/introspect-network.sh`

#### install-with-network-plugin.sh
- **Issue**: "ERROR: Must run as root"
- **Cause**: Installation requires root privileges
- **Solution**: Always run with sudo: `sudo ./scripts/install-with-network-plugin.sh [options]`

### 2. Missing Dependencies

#### Cargo/Rust Not Found
- **Issue**: "ERROR: cargo not found"
- **Solution**:
  1. Install Rust as regular user (not root):
     ```bash
     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
     ```
  2. Load Rust environment:
     ```bash
     source $HOME/.cargo/env
     ```
  3. Re-run installer with sudo

#### OpenVSwitch Not Installed
- **Issue**: "WARNING: openvswitch-switch not installed"
- **Solution**:
  ```bash
  sudo apt-get update
  sudo apt-get install openvswitch-switch
  sudo systemctl enable openvswitch-switch
  sudo systemctl start openvswitch-switch
  ```

#### Python3 YAML Module Missing
- **Issue**: "ERROR: python3-yaml not found (required for --with-ovsbr1)"
- **Solution**:
  ```bash
  sudo apt-get install python3-yaml
  ```

### 3. Network Detection Issues

#### No Primary Interface Found
- **Issue**: introspect-network.sh can't detect primary interface
- **Possible Causes**:
  - System using OVS bridges already
  - No active network interfaces
  - Unusual network configuration
- **Solutions**:
  1. Check current interfaces:
     ```bash
     ip addr show
     ip link show
     ```
  2. Use existing config instead of introspection:
     ```bash
     sudo ./scripts/install-with-network-plugin.sh \
       --network-config config/examples/network-ovs-bridges.yaml \
       --system
     ```

### 4. Installation Failures

#### Build Failures
- **Issue**: Cargo build fails
- **Solutions**:
  1. Check Rust version: `rustc --version` (should be recent)
  2. Update Rust: `rustup update`
  3. Check disk space: `df -h`
  4. Check build logs for specific errors

#### Network Configuration Failures
- **Issue**: "Failed to apply network config"
- **Solutions**:
  1. Check OVS is running:
     ```bash
     sudo systemctl status openvswitch-switch
     ```
  2. Validate config syntax:
     ```bash
     sudo ovs-port-agent show-diff <config-file>
     ```
  3. Check for network conflicts
  4. Review backup directory: `/var/lib/ovs-port-agent/backups/`

### 5. Post-Installation Issues

#### Service Won't Start
- **Issue**: ovs-port-agent service fails to start
- **Solutions**:
  1. Check service status:
     ```bash
     sudo systemctl status ovs-port-agent
     ```
  2. Check logs:
     ```bash
     sudo journalctl -u ovs-port-agent -n 50
     ```
  3. Verify D-Bus policy installed:
     ```bash
     ls -la /etc/dbus-1/system.d/dev.ovs.PortAgent1.conf
     ```

#### Lost Network Connectivity
- **Issue**: Network connectivity lost after installation
- **Solution**: The installer creates backups before changes
  1. Check backup directory:
     ```bash
     ls -la /var/lib/ovs-port-agent/backups/
     ```
  2. Review pre-installation state in backup files
  3. Manual recovery if needed using `ip` and `ovs-vsctl` commands

## Quick Command Reference

### Check System State
```bash
# Network interfaces
ip addr show
ip link show

# OVS state
sudo ovs-vsctl show

# Service status
sudo systemctl status openvswitch-switch
sudo systemctl status ovs-port-agent

# Check logs
sudo journalctl -xe
sudo journalctl -u ovs-port-agent -f
```

### Installation Commands
```bash
# Basic installation with introspection
sudo ./scripts/install-with-network-plugin.sh --introspect --system

# Installation with existing config
sudo ./scripts/install-with-network-plugin.sh \
  --network-config config/examples/network-ovs-bridges.yaml \
  --system

# Installation with Docker bridge
sudo ./scripts/install-with-network-plugin.sh \
  --introspect \
  --with-ovsbr1 \
  --system
```

### Recovery Commands
```bash
# Stop service if running
sudo systemctl stop ovs-port-agent

# Manual OVS cleanup (use with caution)
sudo ovs-vsctl list-br
sudo ovs-vsctl del-br <bridge-name>

# Restart networking
sudo systemctl restart networking
```

## Getting Help

1. Check the main documentation:
   - `README.md`
   - `docs/NETWORK_PLUGIN_GUIDE.md`
   - `QUICK_START_NETWORK_PLUGIN.md`

2. Run scripts with `--help`:
   ```bash
   ./scripts/install-with-network-plugin.sh --help
   ```

3. Enable debug output:
   ```bash
   set -x  # Add to script or run before script
   ```

4. Check example configs in `config/examples/`