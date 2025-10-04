#!/usr/bin/env bash
# Validate NetworkManager OVS bridge compliance
# Based on NetworkManager.dev documentation

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
log_check() { echo -e "${BLUE}[CHECK]${NC} $*"; }

# Default bridge to check
: "${BRIDGE:=ovsbr0}"

# Track validation results
ERRORS=0
WARNINGS=0

# Increment error count
add_error() {
    ((ERRORS++))
    log_error "$1"
}

# Increment warning count
add_warning() {
    ((WARNINGS++))
    log_warn "$1"
}

# Check if command exists
check_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        add_error "$1 command not found"
        return 1
    fi
    return 0
}

# Validate connection exists and is active
validate_connection() {
    local conn_name="$1"
    local conn_type="$2"
    
    log_check "Validating connection: $conn_name (type: $conn_type)"
    
    # Check if connection exists
    if ! nmcli -t -f NAME connection show "$conn_name" >/dev/null 2>&1; then
        add_error "Connection $conn_name does not exist"
        return 1
    fi
    
    # Get connection details
    local conn_info=$(nmcli -t -f TYPE,ACTIVE,STATE connection show "$conn_name" 2>/dev/null)
    
    # Check connection type
    local actual_type=$(echo "$conn_info" | grep "^connection.type:" | cut -d: -f2)
    if [[ "$actual_type" != "$conn_type" ]]; then
        add_error "Connection $conn_name has wrong type: expected $conn_type, got $actual_type"
    fi
    
    # Check if active
    local is_active=$(nmcli -t -f NAME,STATE connection show "$conn_name" | grep -c ":activated$" || true)
    if [[ "$is_active" -eq 0 ]]; then
        add_warning "Connection $conn_name is not active"
    fi
    
    return 0
}

# Validate OVS bridge settings
validate_bridge_settings() {
    local bridge_name="$1"
    
    log_check "Validating OVS bridge settings for $bridge_name"
    
    # Get bridge settings
    local settings=$(nmcli -t connection show "$bridge_name" 2>/dev/null)
    
    # Check STP setting (should be no)
    local stp=$(echo "$settings" | grep "^ovs-bridge.stp:" | cut -d: -f2)
    if [[ "$stp" != "no" ]]; then
        add_warning "Bridge $bridge_name has STP enabled (should be 'no' for OVS)"
    fi
    
    # Check RSTP setting (should be no)
    local rstp=$(echo "$settings" | grep "^ovs-bridge.rstp:" | cut -d: -f2)
    if [[ "$rstp" != "no" ]]; then
        add_warning "Bridge $bridge_name has RSTP enabled (should be 'no' for OVS)"
    fi
    
    # Check autoconnect
    local autoconnect=$(echo "$settings" | grep "^connection.autoconnect:" | cut -d: -f2)
    if [[ "$autoconnect" != "yes" ]]; then
        add_error "Bridge $bridge_name does not have autoconnect enabled"
    fi
    
    # Check autoconnect priority
    local priority=$(echo "$settings" | grep "^connection.autoconnect-priority:" | cut -d: -f2)
    if [[ "$priority" -lt 50 ]]; then
        add_warning "Bridge $bridge_name has low autoconnect priority: $priority"
    fi
}

# Validate port settings
validate_port_settings() {
    local port_name="$1"
    local expected_master="$2"
    
    log_check "Validating OVS port settings for $port_name"
    
    local settings=$(nmcli -t connection show "$port_name" 2>/dev/null)
    
    # Check master
    local master=$(echo "$settings" | grep "^connection.master:" | cut -d: -f2)
    if [[ "$master" != "$expected_master" ]]; then
        add_error "Port $port_name has wrong master: expected $expected_master, got $master"
    fi
    
    # Check slave type
    local slave_type=$(echo "$settings" | grep "^connection.slave-type:" | cut -d: -f2)
    if [[ "$slave_type" != "ovs-bridge" ]]; then
        add_error "Port $port_name has wrong slave-type: expected ovs-bridge, got $slave_type"
    fi
}

# Validate interface settings
validate_interface_settings() {
    local if_name="$1"
    local expected_master="$2"
    
    log_check "Validating OVS interface settings for $if_name"
    
    local settings=$(nmcli -t connection show "$if_name" 2>/dev/null)
    
    # Check master
    local master=$(echo "$settings" | grep "^connection.master:" | cut -d: -f2)
    if [[ "$master" != "$expected_master" ]]; then
        add_error "Interface $if_name has wrong master: expected $expected_master, got $master"
    fi
    
    # Check slave type
    local slave_type=$(echo "$settings" | grep "^connection.slave-type:" | cut -d: -f2)
    if [[ "$slave_type" != "ovs-port" ]]; then
        add_error "Interface $if_name has wrong slave-type: expected ovs-port, got $slave_type"
    fi
    
    # Check interface type for internal port
    local if_type=$(echo "$settings" | grep "^ovs-interface.type:" | cut -d: -f2)
    if [[ "$if_name" == *"-if" && "$if_type" != "internal" ]]; then
        add_error "Interface $if_name should be type 'internal', got '$if_type'"
    fi
}

# Check OVS state
validate_ovs_state() {
    local bridge_name="$1"
    
    log_check "Validating Open vSwitch state for $bridge_name"
    
    if ! ovs-vsctl br-exists "$bridge_name"; then
        add_error "Bridge $bridge_name does not exist in Open vSwitch"
        return 1
    fi
    
    # Get bridge info
    local bridge_info=$(ovs-vsctl show 2>/dev/null | grep -A 10 "Bridge.*$bridge_name")
    
    if [[ -z "$bridge_info" ]]; then
        add_error "Could not get Open vSwitch info for bridge $bridge_name"
        return 1
    fi
    
    # Check for fail_mode
    local fail_mode=$(ovs-vsctl get-fail-mode "$bridge_name" 2>/dev/null || echo "standalone")
    log_info "Bridge fail_mode: $fail_mode"
    
    # Check for controller
    local controller=$(ovs-vsctl get-controller "$bridge_name" 2>/dev/null || echo "none")
    if [[ "$controller" != "none" && -n "$controller" ]]; then
        log_info "Bridge has controller: $controller"
    fi
}

# Check D-Bus service
validate_dbus_service() {
    log_check "Validating D-Bus service"
    
    if ! gdbus introspect --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 >/dev/null 2>&1; then
        add_warning "D-Bus service dev.ovs.PortAgent1 is not available"
    else
        log_info "D-Bus service is running and introspectable"
    fi
}

# Main validation
main() {
    log_info "Starting NetworkManager OVS compliance validation for bridge: $BRIDGE"
    
    # Check prerequisites
    check_command nmcli || exit 1
    check_command ovs-vsctl || exit 1
    
    # Validate bridge connection
    if validate_connection "$BRIDGE" "ovs-bridge"; then
        validate_bridge_settings "$BRIDGE"
    fi
    
    # Validate internal port
    local port_name="${BRIDGE}-port-int"
    if validate_connection "$port_name" "ovs-port"; then
        validate_port_settings "$port_name" "$BRIDGE"
    fi
    
    # Validate interface
    local if_name="${BRIDGE}-if"
    if validate_connection "$if_name" "ovs-interface"; then
        validate_interface_settings "$if_name" "$port_name"
    fi
    
    # Check for uplink ports
    log_check "Checking for uplink ports"
    local uplink_ports=$(nmcli -t -f NAME connection show | grep "^${BRIDGE}-port-" | grep -v -- "-int$" || true)
    
    if [[ -n "$uplink_ports" ]]; then
        while IFS= read -r port; do
            log_info "Found uplink port: $port"
            validate_connection "$port" "ovs-port"
            validate_port_settings "$port" "$BRIDGE"
        done <<< "$uplink_ports"
    else
        log_info "No uplink ports found"
    fi
    
    # Validate OVS state
    validate_ovs_state "$BRIDGE"
    
    # Check D-Bus service
    validate_dbus_service
    
    # Summary
    echo
    log_info "=== Validation Summary ==="
    if [[ $ERRORS -eq 0 ]]; then
        log_info "✅ No errors found"
    else
        log_error "❌ Found $ERRORS errors"
    fi
    
    if [[ $WARNINGS -eq 0 ]]; then
        log_info "✅ No warnings found"
    else
        log_warn "⚠️  Found $WARNINGS warnings"
    fi
    
    # Exit with error if any errors found
    exit $ERRORS
}

# Run main
main "$@"