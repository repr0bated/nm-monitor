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
        # Following NetworkManager documentation Example 20 EXACTLY
        nmcli conn add type ovs-bridge conn.interface "$bridge_name" || {
            log_error "Failed to create bridge $bridge_name"
            return 1
        }
    fi
    
    log_info "OVS bridge $bridge_name configured"
}

# Create internal port for bridge IP assignment
# Following NetworkManager documentation Example 20
create_internal_port() {
    local bridge_name="$1"
    local ip_addr="${2:-}"
    local gateway="${3:-}"
    local port_name="port0"
    local if_name="iface0"
    
    log_info "Creating internal port for bridge $bridge_name (following NM docs)"
    
    # Step 1: Create OVS port (Example 20, line 2)
    log_info "Creating OVS port with controller relationship"
    nmcli conn add type ovs-port conn.interface "$port_name" controller "$bridge_name" || {
        log_error "Failed to create port $port_name"
        return 1
    }
    
    # Step 2: Create OVS interface with IP (Example 20, line 3)
    log_info "Creating OVS interface with IP configuration"
    
    # Build command following documentation exactly
    local cmd=(nmcli conn add type ovs-interface port-type ovs-port 
              conn.interface "$if_name" controller "$port_name")
    
    # Add IP configuration if provided
    if [[ -n "$ip_addr" ]]; then
        cmd+=(ipv4.method manual ipv4.address "$ip_addr")
        if [[ -n "$gateway" ]]; then
            # In the docs, gateway might be set separately or as part of ipv4.address
            cmd+=(ipv4.gateway "$gateway")
        fi
    else
        cmd+=(ipv4.method disabled)
    fi
    
    "${cmd[@]}" || {
        log_error "Failed to create interface $if_name"
        return 1
    }
    
    log_info "Internal port configured per NetworkManager documentation"
}

# Configure IP address - not needed as separate function
# IP should be configured when creating the ovs-interface connection

# Get IP configuration from active connection using introspection
introspect_ip_config() {
    local conn_name="$1"
    local ip_info=""
    
    # Get IPv4 configuration from the active connection
    local ipv4_method=$(nmcli -t -f ipv4.method connection show "$conn_name" 2>/dev/null | cut -d: -f2)
    
    if [[ "$ipv4_method" == "manual" || "$ipv4_method" == "auto" ]]; then
        # Get addresses
        local ipv4_addresses=$(nmcli -t -f ipv4.addresses connection show "$conn_name" 2>/dev/null | cut -d: -f2)
        # Get gateway
        local ipv4_gateway=$(nmcli -t -f ipv4.gateway connection show "$conn_name" 2>/dev/null | cut -d: -f2)
        # Get DNS
        local ipv4_dns=$(nmcli -t -f ipv4.dns connection show "$conn_name" 2>/dev/null | cut -d: -f2)
        
        if [[ -n "$ipv4_addresses" ]]; then
            ip_info="METHOD=$ipv4_method"
            ip_info="$ip_info;ADDRESSES=$ipv4_addresses"
            [[ -n "$ipv4_gateway" ]] && ip_info="$ip_info;GATEWAY=$ipv4_gateway"
            [[ -n "$ipv4_dns" ]] && ip_info="$ip_info;DNS=$ipv4_dns"
        fi
    fi
    
    echo "$ip_info"
}

# Create uplink port
create_uplink_port() {
    local bridge_name="$1"
    local uplink_if="$2"
    local port_name="${bridge_name}-port-${uplink_if}"
    
    log_info "Creating uplink port for interface $uplink_if on bridge $bridge_name"
    
    # Safety check for remote systems
    if [[ -n "${SSH_CONNECTION:-}" ]]; then
        ssh_device=$(ip -4 addr show | grep "inet $(echo "$SSH_CONNECTION" | awk '{print $3}')" | awk '{print $NF}')
        if [[ "$ssh_device" == "$uplink_if" ]]; then
            log_error "WARNING: Interface $uplink_if is being used for SSH!"
            log_error "Modifying it will disconnect your session!"
            if [[ "$NON_INTERACTIVE" != 1 ]]; then
                read -p "Continue anyway? (yes/no) " -r
                if [[ ! "$REPLY" == "yes" ]]; then
                    log_info "Aborting for safety"
                    return 1
                fi
            fi
        fi
    fi
    
    # Check for active connection on uplink interface
    local active_conn=$(nmcli -t -f NAME,DEVICE,TYPE,ACTIVE connection show --active | \
        awk -F: -v dev="$uplink_if" '$2==dev && $3=="802-3-ethernet" && $4=="yes" {print $1; exit}')
    
    # If there's an active connection, introspect its IP configuration
    local migrate_ip=false
    local ip_config=""
    if [[ -n "$active_conn" ]]; then
        log_info "Found active connection '$active_conn' on $uplink_if"
        ip_config=$(introspect_ip_config "$active_conn")
        if [[ -n "$ip_config" ]]; then
            log_info "Detected IP configuration to migrate: $ip_config"
            migrate_ip=true
        fi
    fi
    
    # Create OVS port following Example 21
    log_info "Creating OVS port for uplink (Example 21)"
    nmcli conn add type ovs-port conn.interface "port1" controller "$bridge_name" || {
        log_error "Failed to create uplink port"
        return 1
    }
    
    # Handle ethernet connection - create with enslavement in one command
    local eth_conn_name="${bridge_name}-eth-${uplink_if}"
    
    if [[ -n "$active_conn" ]]; then
        log_info "Migrating active connection '$active_conn' to OVS slave"
        
        # First remove IP configuration from the active connection
        nmcli connection modify "$active_conn" \
            ipv4.method disabled \
            ipv4.addresses "" \
            ipv4.gateway "" \
            ipv6.method disabled || true
        
        # Then modify it to be enslaved in one command
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
        # Create new ethernet connection already enslaved
        if nmcli -t -f NAME connection show "$eth_conn_name" >/dev/null 2>&1; then
            log_info "Ethernet connection $eth_conn_name already exists, recreating..."
            nmcli connection delete "$eth_conn_name" 2>/dev/null || true
        fi
        
        # Create ethernet following Example 21
        log_info "Adding Linux interface to bridge (Example 21)"
        nmcli conn add type ethernet conn.interface "$uplink_if" controller "port1" || {
            log_error "Failed to create ethernet connection"
            return 1
        }
    fi
    
    log_info "Uplink port configured for interface $uplink_if"
    
    # Return whether IP needs to be migrated
    if [[ "$migrate_ip" == "true" ]]; then
        echo "$ip_config"
    fi
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
    
    # Create ALL components before ANY activation
    log_info "Creating complete bridge configuration..."
    
    # Create bridge
    create_ovs_bridge "$BRIDGE"
    
    # Check if we need to migrate IP from uplink
    local migrated_ip=""
    local use_configured_ip=true
    
    # Create uplink port first to check for IP migration
    if [[ -n "$UPLINK" ]]; then
        log_info "Checking uplink $UPLINK for IP configuration to migrate..."
        migrated_ip=$(create_uplink_port "$BRIDGE" "$UPLINK")
        
        if [[ -n "$migrated_ip" ]]; then
            log_info "Will migrate IP configuration from uplink"
            use_configured_ip=false
            
            # Parse the migrated IP info
            local method=$(echo "$migrated_ip" | grep -o 'METHOD=[^;]*' | cut -d= -f2)
            local addresses=$(echo "$migrated_ip" | grep -o 'ADDRESSES=[^;]*' | cut -d= -f2)
            local gateway=$(echo "$migrated_ip" | grep -o 'GATEWAY=[^;]*' | cut -d= -f2)
            local dns=$(echo "$migrated_ip" | grep -o 'DNS=[^;]*' | cut -d= -f2)
            
            # Use migrated configuration
            NM_IP="$addresses"
            NM_GW="$gateway"
        fi
    fi
    
    # Create internal port and interface with appropriate IP configuration
    if [[ "$use_configured_ip" == "true" ]]; then
        # Use manually specified IP configuration
        create_internal_port "$BRIDGE" "$NM_IP" "$NM_GW"
    else
        # Use migrated IP configuration from uplink
        create_internal_port "$BRIDGE" "$NM_IP" "$NM_GW"
    fi
    
    # NOW activate bridge - NetworkManager will handle all slaves atomically
    activate_bridge "$BRIDGE"
    validate_bridge "$BRIDGE"
    
    # Create secondary bridge if requested
    if [[ "$WITH_OVSBR1" == 1 ]]; then
        log_info "Creating secondary bridge ovsbr1"
        
        create_ovs_bridge "ovsbr1"
        create_internal_port "ovsbr1" "$OVSBR1_IP" "$OVSBR1_GW"
        
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