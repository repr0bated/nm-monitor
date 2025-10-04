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
    
    log_info "All prerequisites met"
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
            connection.autoconnect yes \
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
    
    # NetworkManager handles slave activation atomically
    nmcli -w 30 connection up "$bridge_name" || {
        log_warn "Failed to activate bridge $bridge_name"
        # Try to get more information about the failure
        nmcli connection show "$bridge_name" | grep -E "GENERAL|STATE"
        return 1
    }
    
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
    
    # Create primary bridge
    create_ovs_bridge "$BRIDGE"
    create_internal_port "$BRIDGE"
    
    # Configure IP if provided
    if [[ -n "$NM_IP" ]]; then
        configure_ip_address "${BRIDGE}-if" "$NM_IP" "$NM_GW"
    fi
    
    # Add uplink if provided
    if [[ -n "$UPLINK" ]]; then
        create_uplink_port "$BRIDGE" "$UPLINK"
    fi
    
    # Activate bridge
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