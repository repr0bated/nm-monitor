#!/usr/bin/env bash
# NetworkManager-compliant OVS bridge installation script
# Strictly follows NetworkManager.dev documentation

set -euo pipefail

# Default values
: "${BRIDGE:=ovsbr0}"
: "${NM_IP:=}"
: "${NM_GW:=}"
: "${UPLINK:=}"
: "${WITH_OVSBR1:=0}"
: "${OVSBR1_IP:=10.200.0.1/24}"
: "${OVSBR1_GW:=}"
: "${OVSBR1_UPLINK:=}"
: "${SYSTEM_INSTALL:=0}"
: "${FORCE_CLEANUP:=0}"
: "${NON_INTERACTIVE:=0}"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --bridge) BRIDGE="$2"; shift 2 ;;
        --nm-ip) NM_IP="$2"; shift 2 ;;
        --nm-gw) NM_GW="$2"; shift 2 ;;
        --uplink) UPLINK="$2"; shift 2 ;;
        --with-ovsbr1) WITH_OVSBR1=1; shift ;;
        --ovsbr1-ip) OVSBR1_IP="$2"; shift 2 ;;
        --ovsbr1-gw) OVSBR1_GW="$2"; shift 2 ;;
        --ovsbr1-uplink) OVSBR1_UPLINK="$2"; shift 2 ;;
        --system) SYSTEM_INSTALL=1; shift ;;
        --force-cleanup) FORCE_CLEANUP=1; shift ;;
        --non-interactive|-y) NON_INTERACTIVE=1; shift ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --bridge NAME           Bridge name (default: ovsbr0)"
            echo "  --nm-ip IP/MASK         IP address for bridge"
            echo "  --nm-gw GATEWAY         Gateway for bridge"
            echo "  --uplink INTERFACE      Physical interface to attach"
            echo "  --with-ovsbr1           Create secondary bridge"
            echo "  --system                Install and start systemd service"
            echo "  --force-cleanup         Force cleanup of existing connections"
            echo "  --non-interactive, -y   Non-interactive mode"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v nmcli >/dev/null 2>&1; then
        log_error "NetworkManager CLI (nmcli) not found"
        exit 1
    fi
    
    if ! systemctl is-active --quiet NetworkManager; then
        log_error "NetworkManager is not running"
        exit 1
    fi
    
    if ! command -v ovs-vsctl >/dev/null 2>&1; then
        log_error "Open vSwitch not installed"
        exit 1
    fi
    
    # Check OVS service
    if ! systemctl is-active --quiet openvswitch-switch && ! systemctl is-active --quiet openvswitch; then
        log_warn "Open vSwitch service is not running, will start it later"
    fi
    
    log_info "All prerequisites met"
}

# Clean up existing conflicting connections
cleanup_existing_connections() {
    local bridge_name="$1"
    
    log_info "Checking for existing connections..."
    
    # Find all connections related to this bridge
    local related_conns=$(nmcli -t -f NAME,TYPE connection show | grep -E "(^${bridge_name}[:-]|^${bridge_name}$)" | cut -d: -f1)
    
    if [[ -n "$related_conns" ]]; then
        log_warn "Found existing connections that may conflict:"
        echo "$related_conns" | while IFS= read -r conn; do
            echo "  - $conn"
        done
        
        if [[ "$NON_INTERACTIVE" == 1 || "$FORCE_CLEANUP" == 1 ]]; then
            REPLY="y"
        else
            read -p "Delete existing connections? (y/N) " -n 1 -r
            echo
        fi
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo "$related_conns" | while IFS= read -r conn; do
                log_info "Deleting connection: $conn"
                nmcli connection delete "$conn" 2>/dev/null || true
            done
            
            # Also clean up OVS if bridge exists
            if ovs-vsctl br-exists "$bridge_name" 2>/dev/null; then
                log_info "Removing bridge from Open vSwitch"
                ovs-vsctl del-br "$bridge_name" || true
            fi
        else
            log_warn "Keeping existing connections, this may cause conflicts"
        fi
    fi
}

# Create OVS bridge following NetworkManager documentation
create_ovs_bridge() {
    local bridge_name="$1"
    
    log_info "Creating OVS bridge $bridge_name"
    
    # Check if bridge connection already exists
    if nmcli -t -f NAME connection show "$bridge_name" >/dev/null 2>&1; then
        log_info "Bridge connection $bridge_name already exists, updating..."
        
        # Update bridge settings according to NM documentation
        nmcli connection modify "$bridge_name" \
            ovs-bridge.stp no \
            ovs-bridge.rstp no \
            ovs-bridge.mcast-snooping-enable yes \
            connection.autoconnect yes \
            connection.autoconnect-priority 100 || {
            log_error "Failed to modify bridge $bridge_name"
            return 1
        }
    else
        # Create new OVS bridge connection
        nmcli connection add \
            type ovs-bridge \
            con-name "$bridge_name" \
            ifname "$bridge_name" \
            ovs-bridge.stp no \
            ovs-bridge.rstp no \
            ovs-bridge.mcast-snooping-enable yes \
            connection.autoconnect yes \
            connection.autoconnect-priority 100 || {
            log_error "Failed to create bridge $bridge_name"
            return 1
        }
    fi
    
    log_info "OVS bridge $bridge_name configured"
}

# Create internal port for bridge IP assignment
create_internal_port() {
    local bridge_name="$1"
    local port_name="${bridge_name}-port-int"
    local if_name="$bridge_name"
    
    log_info "Creating internal port for bridge $bridge_name"
    
    # Create OVS port
    if nmcli -t -f NAME connection show "$port_name" >/dev/null 2>&1; then
        log_info "Port connection $port_name already exists, updating..."
        
        nmcli connection modify "$port_name" \
            connection.master "$bridge_name" \
            connection.slave-type ovs-bridge \
            connection.autoconnect yes \
            connection.autoconnect-priority 95 || {
            log_error "Failed to modify port $port_name"
            return 1
        }
    else
        nmcli connection add \
            type ovs-port \
            con-name "$port_name" \
            ifname "$if_name" \
            connection.master "$bridge_name" \
            connection.slave-type ovs-bridge \
            connection.autoconnect yes \
            connection.autoconnect-priority 95 || {
            log_error "Failed to create port $port_name"
            return 1
        }
    fi
    
    # Create OVS interface
    local if_conn_name="${bridge_name}-if"
    
    if nmcli -t -f NAME connection show "$if_conn_name" >/dev/null 2>&1; then
        log_info "Interface connection $if_conn_name already exists, updating..."
        
        nmcli connection modify "$if_conn_name" \
            connection.master "$port_name" \
            connection.slave-type ovs-port \
            connection.autoconnect yes \
            connection.autoconnect-priority 95 \
            ovs-interface.type internal || {
            log_error "Failed to modify interface $if_conn_name"
            return 1
        }
    else
        nmcli connection add \
            type ovs-interface \
            con-name "$if_conn_name" \
            ifname "$if_name" \
            connection.master "$port_name" \
            connection.slave-type ovs-port \
            connection.autoconnect no \
            connection.autoconnect-priority 95 \
            ovs-interface.type internal || {
            log_error "Failed to create interface $if_conn_name"
            return 1
        }
    fi
    
    log_info "Internal port configured for bridge $bridge_name"
}

# Configure IP address on bridge interface
configure_ip_address() {
    local if_conn_name="$1"
    local ip_addr="$2"
    local gateway="$3"
    
    log_info "Configuring IP $ip_addr on $if_conn_name"
    
    local args=(
        connection modify "$if_conn_name"
        ipv4.method manual
        ipv4.addresses "$ip_addr"
        ipv6.method disabled
    )
    
    if [[ -n "$gateway" ]]; then
        args+=(ipv4.gateway "$gateway")
    fi
    
    nmcli "${args[@]}" || {
        log_error "Failed to configure IP on $if_conn_name"
        return 1
    }
    
    log_info "IP address configured on $if_conn_name"
}

# Create uplink port
create_uplink_port() {
    local bridge_name="$1"
    local uplink_if="$2"
    local port_name="${bridge_name}-port-${uplink_if}"
    
    log_info "Creating uplink port for interface $uplink_if on bridge $bridge_name"
    
    # Check for active connection on uplink interface
    local active_conn=$(nmcli -t -f NAME,DEVICE,TYPE,ACTIVE connection show --active | \
        awk -F: -v dev="$uplink_if" '$2==dev && $3=="802-3-ethernet" && $4=="yes" {print $1; exit}')
    
    # Create OVS port
    if nmcli -t -f NAME connection show "$port_name" >/dev/null 2>&1; then
        log_info "Port connection $port_name already exists, updating..."
        
        nmcli connection modify "$port_name" \
            connection.master "$bridge_name" \
            connection.slave-type ovs-bridge \
            connection.autoconnect yes \
            connection.autoconnect-priority 90 || {
            log_error "Failed to modify port $port_name"
            return 1
        }
    else
        nmcli connection add \
            type ovs-port \
            con-name "$port_name" \
            ifname "$uplink_if" \
            connection.master "$bridge_name" \
            connection.slave-type ovs-bridge \
            connection.autoconnect yes \
            connection.autoconnect-priority 90 || {
            log_error "Failed to create port $port_name"
            return 1
        }
    fi
    
    # Handle ethernet connection
    local eth_conn_name="${bridge_name}-eth-${uplink_if}"
    
    if [[ -n "$active_conn" ]]; then
        log_info "Migrating active connection '$active_conn' to OVS slave"
        
        # Modify existing connection to be OVS slave
        nmcli connection modify "$active_conn" \
            connection.master "$port_name" \
            connection.slave-type ovs-port \
            connection.autoconnect yes \
            connection.autoconnect-priority 85 || {
            log_error "Failed to modify active connection"
            return 1
        }
        
        # Optionally rename for consistency
        nmcli connection modify "$active_conn" \
            connection.id "$eth_conn_name" || true
    else
        # Create new ethernet connection
        if nmcli -t -f NAME connection show "$eth_conn_name" >/dev/null 2>&1; then
            log_info "Ethernet connection $eth_conn_name already exists, updating..."
            
            nmcli connection modify "$eth_conn_name" \
                connection.master "$port_name" \
                connection.slave-type ovs-port \
                connection.autoconnect yes \
                connection.autoconnect-priority 85 || {
                log_error "Failed to modify ethernet connection"
                return 1
            }
        else
            nmcli connection add \
                type ethernet \
                con-name "$eth_conn_name" \
                ifname "$uplink_if" \
                connection.master "$port_name" \
                connection.slave-type ovs-port \
                connection.autoconnect yes \
                connection.autoconnect-priority 85 \
                802-3-ethernet.auto-negotiate yes || {
                log_error "Failed to create ethernet connection"
                return 1
            }
        fi
    fi
    
    log_info "Uplink port configured for interface $uplink_if"
}

# Activate bridge with atomic handoff
activate_bridge() {
    local bridge_name="$1"
    
    log_info "Activating bridge $bridge_name (atomic handoff)"
    
    # Check if OVS is running
    if ! systemctl is-active --quiet openvswitch-switch && ! systemctl is-active --quiet openvswitch; then
        log_error "Open vSwitch service is not running"
        log_info "Starting Open vSwitch..."
        systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null || {
            log_error "Failed to start Open vSwitch"
            return 1
        }
        sleep 2
    fi
    
    # Check if bridge exists in OVS
    if ! ovs-vsctl br-exists "$bridge_name" 2>/dev/null; then
        log_info "Bridge $bridge_name not in OVS, adding..."
        ovs-vsctl add-br "$bridge_name" || {
            log_error "Failed to add bridge to Open vSwitch"
            return 1
        }
    fi
    
    # Make sure slaves are ready
    log_info "Ensuring slave connections are ready..."
    
    # The slaves should auto-activate with the bridge, but let's make sure they're configured properly
    local port_name="${bridge_name}-port-int"
    local if_name="${bridge_name}-if"
    
    # Check if the connections exist and are properly configured
    if nmcli connection show "$port_name" >/dev/null 2>&1; then
        log_info "Port connection $port_name exists"
    else
        log_error "Port connection $port_name missing!"
        return 1
    fi
    
    if nmcli connection show "$if_name" >/dev/null 2>&1; then
        log_info "Interface connection $if_name exists"
    else
        log_error "Interface connection $if_name missing!"
        return 1
    fi
    
    # First, set autoconnect on the interface now that we're ready
    nmcli connection modify "$if_name" connection.autoconnect yes || true
    
    # Try to activate with a shorter timeout first
    log_info "Attempting to activate bridge..."
    # Use --wait to ensure proper ordering
    if ! nmcli --wait 10 connection up "$bridge_name" 2>&1 | tee /tmp/bridge-activation.log; then
        log_warn "Initial activation failed, checking state..."
        
        # Check current state
        local state=$(nmcli -t -f GENERAL.STATE connection show "$bridge_name" | cut -d: -f2)
        log_info "Bridge state: $state"
        
        if [[ "$state" == "activating" ]]; then
            log_info "Bridge is still activating, waiting..."
            sleep 5
            
            # Check again
            state=$(nmcli -t -f GENERAL.STATE connection show "$bridge_name" | cut -d: -f2)
            if [[ "$state" == "activated" ]]; then
                log_info "Bridge activated successfully"
                return 0
            fi
        fi
        
        # Try to diagnose the issue
        log_warn "Activation failed, diagnosing..."
        nmcli connection show "$bridge_name" | grep -E "GENERAL|STATE|ERROR" || true
        
        # Check journal for errors
        log_info "Checking system logs..."
        journalctl -u NetworkManager -n 20 --no-pager | grep -E "error|fail|ovs" -i || true
        
        # Check if slaves are blocking
        log_info "Checking slave connections..."
        local slaves=$(nmcli -t -f NAME,TYPE connection show | grep -E "ovs-(port|interface)" | cut -d: -f1)
        if [[ -n "$slaves" ]]; then
            for slave in $slaves; do
                local slave_info=$(nmcli -t -f GENERAL.STATE,connection.master connection show "$slave" 2>/dev/null || echo "unknown:unknown")
                local slave_state=$(echo "$slave_info" | cut -d: -f1)
                local slave_master=$(echo "$slave_info" | cut -d: -f2)
                if [[ "$slave_master" == "$bridge_name" ]]; then
                    log_info "  $slave: state=$slave_state"
                fi
            done
        fi
        
        return 1
    fi
    
    log_info "Bridge $bridge_name activated successfully"
}

# Validate bridge topology
validate_bridge() {
    local bridge_name="$1"
    
    log_info "Validating OVS bridge $bridge_name topology"
    
    # Check bridge connection
    if ! nmcli -t -f NAME,STATE connection show "$bridge_name" | grep -q ":activated$"; then
        log_error "Bridge $bridge_name is not active"
        return 1
    fi
    
    # Check internal port
    local port_name="${bridge_name}-port-int"
    if ! nmcli -t -f NAME,STATE connection show "$port_name" 2>/dev/null | grep -q ":activated$"; then
        log_warn "Internal port $port_name is not active"
    fi
    
    # Check interface
    local if_name="${bridge_name}-if"
    if ! nmcli -t -f NAME,STATE connection show "$if_name" 2>/dev/null | grep -q ":activated$"; then
        log_warn "Interface $if_name is not active"
    fi
    
    # Verify OVS state
    if ovs-vsctl br-exists "$bridge_name"; then
        log_info "OVS bridge $bridge_name exists in Open vSwitch"
        ovs-vsctl show | grep -A 5 "Bridge.*$bridge_name"
    else
        log_error "OVS bridge $bridge_name not found in Open vSwitch"
        return 1
    fi
    
    log_info "Bridge $bridge_name validation complete"
}

# Install systemd service
install_systemd_service() {
    log_info "Installing systemd service"
    
    local service_path="/etc/systemd/system/ovs-port-agent.service"
    local binary_path="/usr/local/bin/ovs-port-agent"
    
    # Copy binary
    if [[ -f "target/release/ovs-port-agent" ]]; then
        sudo cp target/release/ovs-port-agent "$binary_path"
        sudo chmod +x "$binary_path"
    else
        log_error "Binary not found at target/release/ovs-port-agent"
        return 1
    fi
    
    # Create systemd service
    cat <<EOF | sudo tee "$service_path" > /dev/null
[Unit]
Description=OVS Port Agent
Documentation=https://github.com/repr0bated/nm-monitor
After=network-online.target NetworkManager.service openvswitch.service
Wants=network-online.target

[Service]
Type=simple
ExecStart=$binary_path run --bridge $BRIDGE
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=ovs-port-agent

# Security settings
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/ovs-port-agent /etc/network
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
EOF

    # Create state directory
    sudo mkdir -p /var/lib/ovs-port-agent
    
    # Reload systemd
    sudo systemctl daemon-reload
    
    if [[ "$SYSTEM_INSTALL" == 1 ]]; then
        sudo systemctl enable --now ovs-port-agent
        sudo systemctl status --no-pager ovs-port-agent || true
    else
        log_info "Service installed. Enable with: systemctl enable --now ovs-port-agent"
    fi
}

# Main installation flow
main() {
    log_info "Starting NetworkManager-compliant OVS bridge installation"
    
    check_prerequisites
    
    # Clean up existing connections if needed
    cleanup_existing_connections "$BRIDGE"
    
    # Ensure OVS service is running
    if ! systemctl is-active --quiet openvswitch-switch && ! systemctl is-active --quiet openvswitch; then
        log_info "Starting Open vSwitch service..."
        systemctl start openvswitch-switch 2>/dev/null || systemctl start openvswitch 2>/dev/null || {
            log_error "Failed to start Open vSwitch service"
            exit 1
        }
        sleep 2
    fi
    
    # Create primary bridge
    create_ovs_bridge "$BRIDGE"
    create_internal_port "$BRIDGE"
    
    # Configure IP if provided
    if [[ -n "$NM_IP" ]]; then
        configure_ip_address "${BRIDGE}-if" "$NM_IP" "$NM_GW"
    fi
    
    # Add uplink BEFORE activation if provided
    if [[ -n "$UPLINK" ]]; then
        log_info "Adding uplink $UPLINK before bridge activation"
        create_uplink_port "$BRIDGE" "$UPLINK"
    fi
    
    # NOW activate bridge with all components ready
    activate_bridge "$BRIDGE"
    validate_bridge "$BRIDGE"
    
    # Create secondary bridge if requested
    if [[ "$WITH_OVSBR1" == 1 ]]; then
        log_info "Creating secondary bridge ovsbr1"
        
        create_ovs_bridge "ovsbr1"
        create_internal_port "ovsbr1"
        
        if [[ -n "$OVSBR1_IP" ]]; then
            configure_ip_address "ovsbr1-if" "$OVSBR1_IP" "$OVSBR1_GW"
        fi
        
        if [[ -n "$OVSBR1_UPLINK" ]]; then
            create_uplink_port "ovsbr1" "$OVSBR1_UPLINK"
        fi
        
        activate_bridge "ovsbr1"
        validate_bridge "ovsbr1"
    fi
    
    # Install systemd service
    if command -v cargo >/dev/null 2>&1 && [[ -f "Cargo.toml" ]]; then
        install_systemd_service
    fi
    
    log_info "Installation complete!"
}

# Run main function
main "$@"