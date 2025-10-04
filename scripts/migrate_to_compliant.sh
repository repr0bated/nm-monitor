#!/usr/bin/env bash
# Migrate existing OVS bridges to NetworkManager-compliant configuration
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
log_step() { echo -e "${BLUE}[STEP]${NC} $*"; }

# Default bridge
: "${BRIDGE:=ovsbr0}"
: "${DRY_RUN:=0}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --bridge) BRIDGE="$2"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        --help)
            echo "Usage: $0 [--bridge BRIDGE] [--dry-run]"
            echo "Migrate OVS bridge to NetworkManager-compliant configuration"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Execute command or print in dry-run mode
execute() {
    if [[ "$DRY_RUN" == 1 ]]; then
        echo "[DRY-RUN] $*"
    else
        "$@"
    fi
}

# Backup current configuration
backup_config() {
    local backup_dir="/tmp/nm-ovs-backup-$(date +%Y%m%d-%H%M%S)"
    
    log_step "Backing up current configuration to $backup_dir"
    
    execute mkdir -p "$backup_dir"
    
    # Export all OVS-related connections
    local ovs_conns=$(nmcli -t -f NAME,TYPE connection show | grep "ovs-" | cut -d: -f1)
    
    if [[ -n "$ovs_conns" ]]; then
        while IFS= read -r conn; do
            log_info "Backing up connection: $conn"
            execute nmcli connection export "$conn" "$backup_dir/${conn}.nmconnection" || true
        done <<< "$ovs_conns"
    fi
    
    # Save OVS configuration
    execute ovs-vsctl show > "$backup_dir/ovs-show.txt" 2>&1 || true
    
    echo "$backup_dir"
}

# Analyze current bridge configuration
analyze_bridge() {
    local bridge_name="$1"
    
    log_step "Analyzing current bridge configuration: $bridge_name"
    
    # Check if bridge exists in OVS
    if ! ovs-vsctl br-exists "$bridge_name" 2>/dev/null; then
        log_error "Bridge $bridge_name does not exist in Open vSwitch"
        return 1
    fi
    
    # Check NetworkManager connections
    local bridge_conn=$(nmcli -t -f NAME,TYPE connection show | grep "^${bridge_name}:ovs-bridge" | cut -d: -f1)
    
    if [[ -z "$bridge_conn" ]]; then
        log_warn "No NetworkManager bridge connection found for $bridge_name"
    else
        log_info "Found bridge connection: $bridge_conn"
        
        # Check compliance
        local issues=()
        
        # Check STP
        local stp=$(nmcli -t -f ovs-bridge.stp connection show "$bridge_conn" | cut -d: -f2)
        if [[ "$stp" != "no" ]]; then
            issues+=("STP is enabled (should be 'no')")
        fi
        
        # Check RSTP
        local rstp=$(nmcli -t -f ovs-bridge.rstp connection show "$bridge_conn" | cut -d: -f2)
        if [[ "$rstp" != "no" ]]; then
            issues+=("RSTP is enabled (should be 'no')")
        fi
        
        # Check autoconnect
        local autoconnect=$(nmcli -t -f connection.autoconnect connection show "$bridge_conn" | cut -d: -f2)
        if [[ "$autoconnect" != "yes" ]]; then
            issues+=("Autoconnect is disabled")
        fi
        
        if [[ ${#issues[@]} -gt 0 ]]; then
            log_warn "Found compliance issues:"
            for issue in "${issues[@]}"; do
                echo "  - $issue"
            done
        else
            log_info "Bridge connection appears compliant"
        fi
    fi
    
    # Check for internal port
    local int_port="${bridge_name}-port-int"
    if ! nmcli -t -f NAME connection show "$int_port" >/dev/null 2>&1; then
        log_warn "Missing internal port connection: $int_port"
    fi
    
    # Check for interface
    local interface="${bridge_name}-if"
    if ! nmcli -t -f NAME connection show "$interface" >/dev/null 2>&1; then
        log_warn "Missing interface connection: $interface"
    fi
}

# Fix bridge configuration
fix_bridge_config() {
    local bridge_name="$1"
    
    log_step "Fixing bridge configuration: $bridge_name"
    
    # Update bridge settings
    if nmcli -t -f NAME connection show "$bridge_name" >/dev/null 2>&1; then
        log_info "Updating bridge connection settings"
        
        execute nmcli connection modify "$bridge_name" \
            ovs-bridge.stp no \
            ovs-bridge.rstp no \
            ovs-bridge.mcast-snooping-enable yes \
            connection.autoconnect yes \
            connection.autoconnect-priority 100
    else
        log_warn "Bridge connection does not exist, skipping modification"
    fi
}

# Create missing connections
create_missing_connections() {
    local bridge_name="$1"
    
    log_step "Creating missing connections for $bridge_name"
    
    # Check/create internal port
    local port_name="${bridge_name}-port-int"
    if ! nmcli -t -f NAME connection show "$port_name" >/dev/null 2>&1; then
        log_info "Creating internal port connection: $port_name"
        
        execute nmcli connection add \
            type ovs-port \
            con-name "$port_name" \
            ifname "$bridge_name" \
            connection.master "$bridge_name" \
            connection.slave-type ovs-bridge \
            connection.autoconnect yes \
            connection.autoconnect-priority 95
    fi
    
    # Check/create interface
    local if_name="${bridge_name}-if"
    if ! nmcli -t -f NAME connection show "$if_name" >/dev/null 2>&1; then
        log_info "Creating interface connection: $if_name"
        
        execute nmcli connection add \
            type ovs-interface \
            con-name "$if_name" \
            ifname "$bridge_name" \
            connection.master "$port_name" \
            connection.slave-type ovs-port \
            connection.autoconnect yes \
            connection.autoconnect-priority 95 \
            ovs-interface.type internal
    fi
}

# Fix connection priorities
fix_priorities() {
    local bridge_name="$1"
    
    log_step "Fixing autoconnect priorities"
    
    # Bridge should have highest priority
    execute nmcli connection modify "$bridge_name" \
        connection.autoconnect-priority 100 || true
    
    # Internal port
    execute nmcli connection modify "${bridge_name}-port-int" \
        connection.autoconnect-priority 95 || true
    
    # Interface
    execute nmcli connection modify "${bridge_name}-if" \
        connection.autoconnect-priority 95 || true
    
    # Find and fix uplink ports
    local uplink_ports=$(nmcli -t -f NAME connection show | grep "^${bridge_name}-port-" | grep -v -- "-int$" || true)
    
    if [[ -n "$uplink_ports" ]]; then
        while IFS= read -r port; do
            log_info "Fixing priority for uplink port: $port"
            execute nmcli connection modify "$port" \
                connection.autoconnect-priority 90 || true
            
            # Find associated ethernet connections
            local eth_conns=$(nmcli -t -f NAME,connection.master connection show | grep ":${port}$" | cut -d: -f1)
            if [[ -n "$eth_conns" ]]; then
                while IFS= read -r eth; do
                    log_info "Fixing priority for ethernet: $eth"
                    execute nmcli connection modify "$eth" \
                        connection.autoconnect-priority 85 || true
                done <<< "$eth_conns"
            fi
        done <<< "$uplink_ports"
    fi
}

# Restart connections
restart_connections() {
    local bridge_name="$1"
    
    if [[ "$DRY_RUN" == 1 ]]; then
        log_info "[DRY-RUN] Would restart bridge connections"
        return
    fi
    
    log_step "Restarting bridge connections"
    
    # Deactivate in reverse order
    log_info "Deactivating connections..."
    nmcli connection down "${bridge_name}-if" 2>/dev/null || true
    nmcli connection down "${bridge_name}-port-int" 2>/dev/null || true
    nmcli connection down "$bridge_name" 2>/dev/null || true
    
    # Wait a moment
    sleep 2
    
    # Activate bridge (slaves will come up atomically)
    log_info "Activating bridge (atomic handoff)..."
    execute nmcli -w 30 connection up "$bridge_name"
}

# Main migration flow
main() {
    log_info "Starting NetworkManager OVS bridge migration"
    
    if [[ "$DRY_RUN" == 1 ]]; then
        log_warn "Running in DRY-RUN mode - no changes will be made"
    fi
    
    # Backup current config
    local backup_dir=$(backup_config)
    log_info "Configuration backed up to: $backup_dir"
    
    # Analyze current state
    analyze_bridge "$BRIDGE"
    
    # Fix bridge configuration
    fix_bridge_config "$BRIDGE"
    
    # Create missing connections
    create_missing_connections "$BRIDGE"
    
    # Fix priorities
    fix_priorities "$BRIDGE"
    
    # Restart if not dry-run
    if [[ "$DRY_RUN" == 0 ]]; then
        read -p "Restart bridge connections now? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            restart_connections "$BRIDGE"
        else
            log_info "Skipping restart. Run 'nmcli connection up $BRIDGE' when ready."
        fi
    fi
    
    # Validate
    log_step "Validation"
    if [[ "$DRY_RUN" == 0 ]]; then
        if [[ -x "./scripts/validate_nm_compliance.sh" ]]; then
            ./scripts/validate_nm_compliance.sh --bridge "$BRIDGE" || true
        else
            log_info "Run validate_nm_compliance.sh to check the configuration"
        fi
    fi
    
    log_info "Migration complete!"
    log_info "Backup saved to: $backup_dir"
}

# Run main
main "$@"