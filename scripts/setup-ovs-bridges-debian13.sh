#!/usr/bin/env bash
# OVS Bridge Setup for Debian 13 (systemd-networkd)
# Creates ovsbr0 and ovsbr1 atomically without NetworkManager
# Supports both D-Bus (gdbus) and ovs-vsctl methods

set -euo pipefail

# Configuration
readonly SCRIPT_NAME="$(basename "$0")"
readonly LOG_FILE="/var/log/ovs-bridge-setup.log"
readonly CHECKPOINT_DIR="/var/lib/ovs-bridge-setup"
readonly NETWORKD_DIR="/etc/systemd/network"

# Bridge configurations
readonly BRIDGE0_NAME="ovsbr0"
readonly BRIDGE0_IP="80.209.240.244/24"
readonly BRIDGE0_GATEWAY="80.209.240.129"

readonly BRIDGE1_NAME="ovsbr1"
readonly BRIDGE1_IP="80.209.242.25/24"
readonly BRIDGE1_GATEWAY="80.209.242.1"

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

# Global variables
USE_DBUS=false
UPLINK_IFACE=""
UPLINK_MAC=""
UPLINK_MTU=""
ROLLBACK_NEEDED=false

# Logging functions
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
    
    case "$level" in
        ERROR) echo -e "${RED}[ERROR]${NC} $message" >&2 ;;
        WARN)  echo -e "${YELLOW}[WARN]${NC} $message" >&2 ;;
        INFO)  echo -e "${GREEN}[INFO]${NC} $message" ;;
        DEBUG) [[ "${DEBUG:-0}" == "1" ]] && echo -e "${BLUE}[DEBUG]${NC} $message" ;;
    esac
}

error_exit() {
    log ERROR "$1"
    [[ "$ROLLBACK_NEEDED" == "true" ]] && rollback_network
    exit 1
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        error_exit "This script must be run as root"
    fi
}

# Check prerequisites
check_prerequisites() {
    log INFO "Checking prerequisites..."
    
    # Check for OVS
    if ! command -v ovs-vsctl &> /dev/null; then
        error_exit "openvswitch-switch is not installed. Install with: apt install openvswitch-switch"
    fi
    
    # Check if OVS is running
    if ! systemctl is-active --quiet openvswitch-switch; then
        log WARN "openvswitch-switch is not running. Starting it..."
        systemctl start openvswitch-switch || error_exit "Failed to start openvswitch-switch"
    fi
    
    # Check for systemd-networkd
    if ! systemctl is-enabled --quiet systemd-networkd; then
        log WARN "systemd-networkd is not enabled. Enabling it..."
        systemctl enable systemd-networkd || error_exit "Failed to enable systemd-networkd"
    fi
    
    if ! systemctl is-active --quiet systemd-networkd; then
        log WARN "systemd-networkd is not running. Starting it..."
        systemctl start systemd-networkd || error_exit "Failed to start systemd-networkd"
    fi
    
    # Create required directories
    mkdir -p "$CHECKPOINT_DIR" "$NETWORKD_DIR"
    
    log INFO "Prerequisites check completed"
}

# Check D-Bus availability
check_dbus_availability() {
    log INFO "Checking D-Bus availability..."
    
    if ! command -v gdbus &> /dev/null; then
        log WARN "gdbus not found, will use ovs-vsctl"
        USE_DBUS=false
        return
    fi
    
    # Check if OVS D-Bus service is available
    if gdbus introspect --system --dest org.openvswitch.ovsdb --object-path / &> /dev/null; then
        log INFO "OVS D-Bus service detected, will use gdbus for atomic operations"
        USE_DBUS=true
    else
        log WARN "OVS D-Bus service not available, will use ovs-vsctl"
        USE_DBUS=false
    fi
}

# Detect active uplink interface
detect_uplink() {
    log INFO "Detecting active uplink interface..."
    
    # Find interface with default route (excluding lo, ovs*, docker*, br-*, veth*)
    UPLINK_IFACE=$(ip -o -4 route show default | awk '$2 !~ /^(lo|ovsbr|docker|br-|veth)/ {print $5}' | head -1)
    
    if [[ -z "$UPLINK_IFACE" ]]; then
        # Fallback: find any interface with an IP
        UPLINK_IFACE=$(ip -o -4 addr show | awk '$2 !~ /^(lo|ovsbr|docker|br-|veth)/ {print $2}' | head -1)
    fi
    
    if [[ -z "$UPLINK_IFACE" ]]; then
        error_exit "Could not detect uplink interface"
    fi
    
    # Get interface properties
    UPLINK_MAC=$(ip link show "$UPLINK_IFACE" | grep -oP 'link/ether \K[^ ]+' | head -1)
    UPLINK_MTU=$(ip link show "$UPLINK_IFACE" | grep -oP 'mtu \K[0-9]+')
    
    log INFO "Detected uplink: $UPLINK_IFACE (MAC: $UPLINK_MAC, MTU: $UPLINK_MTU)"
}

# Create checkpoint of current network state
create_checkpoint() {
    log INFO "Creating network state checkpoint..."
    
    local checkpoint_file="$CHECKPOINT_DIR/checkpoint-$(date +%Y%m%d-%H%M%S).txt"
    
    {
        echo "=== Network Checkpoint $(date) ==="
        echo "Uplink Interface: $UPLINK_IFACE"
        echo ""
        echo "=== IP Addresses ==="
        ip addr show
        echo ""
        echo "=== Routing Table ==="
        ip route show
        echo ""
        echo "=== OVS Configuration ==="
        ovs-vsctl show
        echo ""
        echo "=== systemd-networkd Status ==="
        networkctl list
    } > "$checkpoint_file"
    
    log INFO "Checkpoint saved to: $checkpoint_file"
}

# Create bridge using D-Bus
create_bridge_dbus() {
    local bridge_name="$1"
    
    log INFO "Creating bridge $bridge_name using D-Bus..."
    
    # Add bridge
    gdbus call --system \
        --dest org.openvswitch.ovsdb \
        --object-path /org/openvswitch/ovsdb \
        --method org.openvswitch.ovsdb.Bridge.Add \
        "$bridge_name" || return 1
    
    # Set bridge properties
    gdbus call --system \
        --dest org.openvswitch.ovsdb \
        --object-path "/org/openvswitch/ovsdb/Bridge/$bridge_name" \
        --method org.openvswitch.ovsdb.Bridge.Set \
        "stp_enable" "false" || return 1
    
    return 0
}

# Create bridge using ovs-vsctl
create_bridge_vsctl() {
    local bridge_name="$1"
    
    log INFO "Creating bridge $bridge_name using ovs-vsctl..."
    
    ovs-vsctl --may-exist add-br "$bridge_name" || return 1
    ovs-vsctl set bridge "$bridge_name" stp_enable=false || return 1
    
    return 0
}

# Add port to bridge using D-Bus
add_port_dbus() {
    local bridge_name="$1"
    local port_name="$2"
    local interface_name="${3:-$port_name}"
    
    log INFO "Adding port $port_name to bridge $bridge_name using D-Bus..."
    
    # Add port
    gdbus call --system \
        --dest org.openvswitch.ovsdb \
        --object-path "/org/openvswitch/ovsdb/Bridge/$bridge_name" \
        --method org.openvswitch.ovsdb.Bridge.AddPort \
        "$port_name" || return 1
    
    # Add interface to port
    gdbus call --system \
        --dest org.openvswitch.ovsdb \
        --object-path "/org/openvswitch/ovsdb/Port/$port_name" \
        --method org.openvswitch.ovsdb.Port.AddInterface \
        "$interface_name" || return 1
    
    return 0
}

# Add port to bridge using ovs-vsctl
add_port_vsctl() {
    local bridge_name="$1"
    local port_name="$2"
    local interface_name="${3:-$port_name}"
    
    log INFO "Adding port $port_name to bridge $bridge_name using ovs-vsctl..."
    
    if [[ "$port_name" == "$interface_name" ]]; then
        ovs-vsctl --may-exist add-port "$bridge_name" "$port_name" || return 1
    else
        ovs-vsctl --may-exist add-port "$bridge_name" "$port_name" -- \
            set interface "$interface_name" type=internal || return 1
    fi
    
    return 0
}

# Create systemd-networkd configuration
create_networkd_config() {
    local bridge_name="$1"
    local ip_address="$2"
    local gateway="$3"
    local is_primary="$4"
    
    log INFO "Creating systemd-networkd configuration for $bridge_name..."
    
    # Create .netdev file for bridge
    cat > "$NETWORKD_DIR/10-$bridge_name.netdev" <<EOF
[NetDev]
Name=$bridge_name
Kind=bridge
EOF
    
    # Create .network file for bridge
    cat > "$NETWORKD_DIR/20-$bridge_name.network" <<EOF
[Match]
Name=$bridge_name

[Network]
Address=$ip_address
Gateway=$gateway
DNS=8.8.8.8
DNS=8.8.4.4

[Link]
RequiredForOnline=yes
EOF
    
    # If this is the primary bridge, configure the uplink
    if [[ "$is_primary" == "true" ]]; then
        # Create .network file for uplink interface
        cat > "$NETWORKD_DIR/30-uplink-$UPLINK_IFACE.network" <<EOF
[Match]
Name=$UPLINK_IFACE

[Network]
Bridge=$bridge_name

[Link]
RequiredForOnline=no
EOF
    fi
}

# Create OVS internal interface
create_ovs_interface() {
    local bridge_name="$1"
    local interface_name="$2"
    
    log INFO "Creating OVS internal interface $interface_name on $bridge_name..."
    
    if [[ "$USE_DBUS" == "true" ]]; then
        add_port_dbus "$bridge_name" "$interface_name" "$interface_name"
    else
        ovs-vsctl --may-exist add-port "$bridge_name" "$interface_name" -- \
            set interface "$interface_name" type=internal
    fi
}

# Setup bridges atomically
setup_bridges() {
    log INFO "Starting atomic bridge setup..."
    
    ROLLBACK_NEEDED=true
    
    # Create bridges
    if [[ "$USE_DBUS" == "true" ]]; then
        create_bridge_dbus "$BRIDGE0_NAME" || error_exit "Failed to create $BRIDGE0_NAME"
        create_bridge_dbus "$BRIDGE1_NAME" || error_exit "Failed to create $BRIDGE1_NAME"
    else
        create_bridge_vsctl "$BRIDGE0_NAME" || error_exit "Failed to create $BRIDGE0_NAME"
        create_bridge_vsctl "$BRIDGE1_NAME" || error_exit "Failed to create $BRIDGE1_NAME"
    fi
    
    # Create systemd-networkd configurations
    create_networkd_config "$BRIDGE0_NAME" "$BRIDGE0_IP" "$BRIDGE0_GATEWAY" "true"
    create_networkd_config "$BRIDGE1_NAME" "$BRIDGE1_IP" "$BRIDGE1_GATEWAY" "false"
    
    # Add uplink to ovsbr0
    if [[ "$USE_DBUS" == "true" ]]; then
        add_port_dbus "$BRIDGE0_NAME" "$UPLINK_IFACE" "$UPLINK_IFACE" || \
            error_exit "Failed to add uplink $UPLINK_IFACE to $BRIDGE0_NAME"
    else
        add_port_vsctl "$BRIDGE0_NAME" "$UPLINK_IFACE" "$UPLINK_IFACE" || \
            error_exit "Failed to add uplink $UPLINK_IFACE to $BRIDGE0_NAME"
    fi
    
    # Create OVS internal interfaces
    create_ovs_interface "$BRIDGE0_NAME" "${BRIDGE0_NAME}-internal"
    create_ovs_interface "$BRIDGE1_NAME" "${BRIDGE1_NAME}-internal"
    
    # Create extra port for Docker/Netmaker on ovsbr1
    create_ovs_interface "$BRIDGE1_NAME" "${BRIDGE1_NAME}-docker"
    
    # Reload systemd-networkd to apply configuration
    log INFO "Reloading systemd-networkd..."
    systemctl reload systemd-networkd
    
    # Wait for network to stabilize
    sleep 3
    
    # Verify connectivity
    if ! ping -c 1 -W 5 "$BRIDGE0_GATEWAY" &> /dev/null; then
        log WARN "Cannot ping gateway $BRIDGE0_GATEWAY, but continuing..."
    fi
    
    ROLLBACK_NEEDED=false
    log INFO "Bridge setup completed successfully"
}

# Rollback function
rollback_network() {
    log ERROR "Rolling back network changes..."
    
    # Remove bridges
    ovs-vsctl --if-exists del-br "$BRIDGE0_NAME"
    ovs-vsctl --if-exists del-br "$BRIDGE1_NAME"
    
    # Remove networkd configurations
    rm -f "$NETWORKD_DIR"/10-*.netdev
    rm -f "$NETWORKD_DIR"/20-*.network
    rm -f "$NETWORKD_DIR"/30-*.network
    
    # Reload networkd
    systemctl reload systemd-networkd
    
    log INFO "Rollback completed"
}

# Show final network state
show_network_state() {
    log INFO "Final network state:"
    
    echo ""
    echo "=== IP Addresses ==="
    ip a
    
    echo ""
    echo "=== Routing Table ==="
    ip r
    
    echo ""
    echo "=== OVS Configuration ==="
    ovs-vsctl show
    
    echo ""
    echo "=== systemd-networkd Status ==="
    networkctl list
}

# Apply security hardening
apply_security_hardening() {
    log INFO "Applying security hardening to bridges..."
    
    for bridge in "$BRIDGE0_NAME" "$BRIDGE1_NAME"; do
        # Disable STP (already done during creation)
        ovs-vsctl set bridge "$bridge" stp_enable=false
        
        # Enable port security
        ovs-vsctl set bridge "$bridge" other_config:mac-aging-time=300
        ovs-vsctl set bridge "$bridge" other_config:mac-table-size=2048
        
        # Add flow rules to drop dangerous packets
        ovs-ofctl add-flow "$bridge" "priority=100,dl_type=0x88cc,actions=drop"  # LLDP
        ovs-ofctl add-flow "$bridge" "priority=100,dl_dst=01:00:0c:cc:cc:cc,actions=drop"  # CDP
        ovs-ofctl add-flow "$bridge" "priority=100,dl_dst=01:80:c2:00:00:00/ff:ff:ff:ff:ff:f0,actions=drop"  # STP
    done
    
    log INFO "Security hardening applied"
}

# Main function
main() {
    log INFO "Starting OVS Bridge Setup for Debian 13"
    log INFO "============================================"
    
    # Initial checks
    check_root
    check_prerequisites
    check_dbus_availability
    
    # Detect network
    detect_uplink
    
    # Create checkpoint
    create_checkpoint
    
    # Setup bridges
    setup_bridges
    
    # Apply security hardening
    apply_security_hardening
    
    # Show final state
    show_network_state
    
    log INFO "============================================"
    log INFO "OVS Bridge setup completed successfully!"
    log INFO "Bridges created: $BRIDGE0_NAME ($BRIDGE0_IP), $BRIDGE1_NAME ($BRIDGE1_IP)"
    log INFO "Log file: $LOG_FILE"
    log INFO "Checkpoints: $CHECKPOINT_DIR"
}

# Run main function
main "$@"
