#!/usr/bin/env bash
# OVS Bridge Setup for Debian 13 using nm-monitor plugin system
# Creates ovsbr0 and ovsbr1 using declarative configuration

set -euo pipefail

# Configuration
readonly SCRIPT_NAME="$(basename "$0")"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(dirname "$SCRIPT_DIR")"
readonly CONFIG_FILE="${REPO_ROOT}/config/examples/debian13-ovs-bridges.yaml"
readonly CUSTOM_CONFIG="${1:-}"

# Colors
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly RED='\033[0;31m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

# Functions
log() {
    echo -e "$1" >&2
}

error_exit() {
    log "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    error_exit "This script must be run as root"
fi

log "${BLUE}========================================${NC}"
log "${BLUE} OVS Bridge Setup for Debian 13${NC}"
log "${BLUE} Using nm-monitor Plugin System${NC}"
log "${BLUE}========================================${NC}"
echo ""

# Check if nm-monitor is installed
if ! command -v ovs-port-agent &> /dev/null; then
    log "${YELLOW}[INFO]${NC} nm-monitor not installed, installing now..."
    
    # Run the install script with introspection
    if [[ -f "${SCRIPT_DIR}/install-with-network-plugin.sh" ]]; then
        log "${YELLOW}[INFO]${NC} Running nm-monitor installation with introspection..."
        "${SCRIPT_DIR}/install-with-network-plugin.sh" --introspect --system || \
            error_exit "Failed to install nm-monitor"
    else
        error_exit "install-with-network-plugin.sh not found"
    fi
else
    log "${GREEN}[INFO]${NC} nm-monitor is already installed"
fi

# Determine config file to use
if [[ -n "$CUSTOM_CONFIG" ]]; then
    if [[ -f "$CUSTOM_CONFIG" ]]; then
        CONFIG_TO_USE="$CUSTOM_CONFIG"
        log "${GREEN}[INFO]${NC} Using custom config: $CUSTOM_CONFIG"
    else
        error_exit "Custom config file not found: $CUSTOM_CONFIG"
    fi
else
    # Update the example config with detected interface
    log "${YELLOW}[INFO]${NC} Detecting primary network interface..."
    
    # Find interface with default route
    PRIMARY_IFACE=$(ip -o -4 route show default | awk '$2 !~ /^(lo|ovsbr|docker|br-|veth)/ {print $5}' | head -1)
    
    if [[ -z "$PRIMARY_IFACE" ]]; then
        # Fallback: find any interface with IP
        PRIMARY_IFACE=$(ip -o -4 addr show | awk '$2 !~ /^(lo|ovsbr|docker|br-|veth)/ {print $2}' | head -1)
    fi
    
    if [[ -z "$PRIMARY_IFACE" ]]; then
        error_exit "Could not detect primary network interface"
    fi
    
    log "${GREEN}[INFO]${NC} Detected primary interface: $PRIMARY_IFACE"
    
    # Create temporary config with detected interface
    TEMP_CONFIG="/tmp/ovs-bridges-config-$$.yaml"
    sed "s/ens1/${PRIMARY_IFACE}/g" "$CONFIG_FILE" > "$TEMP_CONFIG"
    CONFIG_TO_USE="$TEMP_CONFIG"
    
    # Show the configuration
    log "${BLUE}[INFO]${NC} Configuration to apply:"
    echo ""
    cat "$CONFIG_TO_USE"
    echo ""
fi

# Show current network state
log "${BLUE}[INFO]${NC} Current network state:"
echo ""
ip -brief addr show
echo ""

# Query current state
log "${YELLOW}[INFO]${NC} Querying current network state..."
ovs-port-agent query-state --plugin net || true
echo ""

# Show what will change
log "${YELLOW}[INFO]${NC} Calculating changes..."
echo ""
ovs-port-agent show-diff "$CONFIG_TO_USE" || error_exit "Failed to calculate diff"
echo ""

# Ask for confirmation
read -p "Apply these changes? This will reconfigure your network. [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    [[ -f "$TEMP_CONFIG" ]] && rm -f "$TEMP_CONFIG"
    log "${YELLOW}[INFO]${NC} Operation cancelled"
    exit 0
fi

# Apply the configuration
log "${GREEN}[INFO]${NC} Applying network configuration..."
if ovs-port-agent apply-state "$CONFIG_TO_USE"; then
    log "${GREEN}[SUCCESS]${NC} Network configuration applied successfully!"
else
    error_exit "Failed to apply network configuration"
fi

# Clean up temp config
[[ -f "$TEMP_CONFIG" ]] && rm -f "$TEMP_CONFIG"

# Show final state
echo ""
log "${BLUE}[INFO]${NC} Final network state:"
echo ""

log "${YELLOW}=== IP Addresses ===${NC}"
ip addr show
echo ""

log "${YELLOW}=== Routing Table ===${NC}"
ip route show
echo ""

log "${YELLOW}=== OVS Configuration ===${NC}"
ovs-vsctl show
echo ""

log "${YELLOW}=== Plugin State ===${NC}"
ovs-port-agent query-state --plugin net | head -50
echo ""

log "${GREEN}========================================${NC}"
log "${GREEN} OVS Bridge Setup Complete!${NC}"
log "${GREEN}========================================${NC}"
log ""
log "Bridges created:"
log "  • ovsbr0: 80.209.240.244/24 (gateway: 80.209.240.129)"
log "  • ovsbr1: 80.209.242.25/24 (gateway: 80.209.242.1)"
log ""
log "Your uplink ($PRIMARY_IFACE) is now attached to ovsbr0"
log ""
log "To modify configuration later:"
log "  1. Edit: $CONFIG_FILE"
log "  2. Apply: sudo ovs-port-agent apply-state <config.yaml>"
log ""
