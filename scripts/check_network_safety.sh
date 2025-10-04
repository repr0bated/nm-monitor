#!/usr/bin/env bash
# Check network configuration safety before OVS bridge setup
# Helps identify which interfaces are safe to modify

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_safe() { echo -e "${GREEN}[SAFE]${NC} $*"; }
log_unsafe() { echo -e "${RED}[UNSAFE]${NC} $*"; }

echo "=== Network Safety Check ==="
echo

# 1. Check all network interfaces
log_info "Network interfaces:"
ip -br link show | grep -v "^lo" | while read -r iface state rest; do
    echo "  $iface: $state"
done
echo

# 2. Check active connections
log_info "Active NetworkManager connections:"
nmcli -t -f NAME,DEVICE,TYPE,STATE connection show --active | while IFS=: read -r name device type state; do
    if [[ "$state" == "activated" ]]; then
        # Get IP if available
        ip=$(nmcli -t -f IP4.ADDRESS connection show "$name" 2>/dev/null | grep -v "^$" | cut -d: -f2 | cut -d/ -f1 | head -1)
        gw=$(nmcli -t -f IP4.GATEWAY connection show "$name" 2>/dev/null | grep -v "^$" | cut -d: -f2 | head -1)
        
        echo "  Device: $device"
        echo "    Connection: $name"
        echo "    Type: $type"
        [[ -n "$ip" ]] && echo "    IP: $ip"
        [[ -n "$gw" && "$gw" != "--" ]] && echo "    Gateway: $gw"
        echo
    fi
done

# 3. Check SSH connection
if [[ -n "${SSH_CONNECTION:-}" ]]; then
    log_warn "SSH connection detected!"
    ssh_client_ip=$(echo "$SSH_CONNECTION" | awk '{print $1}')
    ssh_client_port=$(echo "$SSH_CONNECTION" | awk '{print $2}')
    ssh_server_ip=$(echo "$SSH_CONNECTION" | awk '{print $3}')
    ssh_server_port=$(echo "$SSH_CONNECTION" | awk '{print $4}')
    
    echo "  Client: $ssh_client_ip:$ssh_client_port"
    echo "  Server: $ssh_server_ip:$ssh_server_port"
    
    # Find which interface has the SSH server IP
    ssh_device=$(ip -4 addr show | grep "inet $ssh_server_ip" | awk '{print $NF}')
    if [[ -n "$ssh_device" ]]; then
        log_unsafe "SSH is using interface: $ssh_device - DO NOT MODIFY THIS INTERFACE!"
    fi
    echo
fi

# 4. Check default route
log_info "Default route:"
default_dev=$(ip route | grep "^default" | grep -o "dev [^ ]*" | awk '{print $2}' | head -1)
if [[ -n "$default_dev" ]]; then
    echo "  Default gateway via: $default_dev"
    log_warn "Modifying $default_dev may cause loss of connectivity"
fi
echo

# 5. Determine safe interfaces
log_info "Safety assessment:"
ip -br link show | grep -v "^lo" | while read -r iface state rest; do
    # Skip virtual interfaces
    if [[ "$iface" =~ ^(docker|veth|virbr|ovs-system) ]]; then
        continue
    fi
    
    # Check if interface is used for SSH
    if [[ -n "${ssh_device:-}" && "$iface" == "$ssh_device" ]]; then
        log_unsafe "$iface - Used for current SSH connection"
        continue
    fi
    
    # Check if interface has default route
    if [[ -n "${default_dev:-}" && "$iface" == "$default_dev" ]]; then
        log_warn "  $iface - Has default route (risky to modify)"
        continue
    fi
    
    # Check if interface has any IP
    if ip addr show "$iface" | grep -q "inet "; then
        ip=$(ip -4 addr show "$iface" | grep "inet " | awk '{print $2}' | head -1)
        log_warn "  $iface - Has IP address: $ip"
    else
        log_safe "$iface - No IP configured (safe to use as uplink)"
    fi
done

echo
log_info "Recommendations:"
echo "1. For remote systems, create the bridge without an uplink first"
echo "2. Configure the bridge IP manually"
echo "3. Test connectivity before adding uplinks"
echo "4. Have console/IPMI access ready as backup"
echo
echo "Safe command for remote system:"
echo "  ./scripts/setup_ovs_bridge_nm.sh ovsbr0 \"\" <IP/MASK> <GATEWAY>"
echo
echo "Then later add uplink if needed:"
echo "  nmcli c add type ovs-port con-name ovsbr0-port-ethX ifname ethX master ovsbr0 slave-type ovs-bridge"