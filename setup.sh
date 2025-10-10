#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# OVS Port Agent Complete Setup Script
# ============================================================================
# This script handles the complete setup of the ovs-port-agent system:
# - Installs Rust and dependencies
# - Builds the project
# - Installs the binary and configuration
# - Sets up the service
# - Provides rollback capabilities

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}" && pwd -P)

echo "🚀 OVS Port Agent Complete Setup"
echo "================================"

# ============================================================================
# 1. ENVIRONMENT SETUP
# ============================================================================

echo "🔧 Phase 1: Setting up environment"
echo "-----------------------------------"

# Install Rust if not present
if ! command -v rustc >/dev/null 2>&1; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
else
    echo "✅ Rust already installed"
fi

# Install system dependencies
echo "Installing system dependencies..."
if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y build-essential pkg-config libssl-dev
    sudo apt-get install -y network-manager openvswitch-switch
elif command -v yum >/dev/null 2>&1; then
    sudo yum groupinstall -y "Development Tools"
    sudo yum install -y pkgconfig openssl-devel
    sudo yum install -y NetworkManager openvswitch
elif command -v pacman >/dev/null 2>&1; then
    sudo pacman -S --needed base-devel pkg-config openssl
    sudo pacman -S --needed networkmanager openvswitch
fi

# ============================================================================
# 2. BUILD PROJECT
# ============================================================================

echo "🔨 Phase 2: Building project"
echo "----------------------------"

cd "${REPO_ROOT}"

# Build release binary
echo "Building release binary..."
cargo build --release

# Run tests
echo "Running tests..."
cargo test

# Check formatting and linting
echo "Checking code quality..."
cargo fmt -- --check
cargo clippy -- -D warnings

# ============================================================================
# 3. INSTALLATION
# ============================================================================

echo "📦 Phase 3: Installation"
echo "-----------------------"

# Run the installation script
echo "Running installation script..."
sudo ./scripts/install.sh --uplink enp2s0 --with-ovsbr1 --system

# ============================================================================
# 4. VERIFICATION
# ============================================================================

echo "✅ Phase 4: Verification"
echo "----------------------"

# Check service status
echo "Checking service status..."
sudo systemctl status ovs-port-agent --no-pager -l

# Check NetworkManager configuration
echo "Checking NetworkManager configuration..."
nmcli connection show | grep ovs

# Check OVS bridges
echo "Checking OVS bridges..."
ovs-vsctl show

# Test D-Bus API
echo "Testing D-Bus API..."
gdbus call --system --dest dev.ovs.PortAgent1 --object-path /dev/ovs/PortAgent1 --method dev.ovs.PortAgent1.ping

# ============================================================================
# 5. SETUP COMPLETE
# ============================================================================

echo "🎉 Phase 5: Setup Complete"
echo "========================="

echo ""
echo "✅ OVS Port Agent setup completed successfully!"
echo ""
echo "📋 What was installed:"
echo "  • ovs-port-agent binary: /usr/local/bin/ovs-port-agent"
echo "  • Configuration: /etc/ovs-port-agent/config.toml"
echo "  • Service: ovs-port-agent (enabled and started)"
echo "  • OVS bridges: ovsbr0 (with uplink), ovsbr1 (isolated)"
echo "  • D-Bus API: dev.ovs.PortAgent1"
echo ""
echo "🔧 Key Features:"
echo "  • Proactive vi{VMID} interface creation"
echo "  • Zero connectivity interruption installation"
echo "  • Comprehensive rollback capabilities"
echo "  • NetworkManager atomic handover"
echo "  • FUSE filesystem integration"
echo ""
echo "📚 Usage:"
echo "  • Service management: sudo systemctl status/start/stop ovs-port-agent"
echo "  • Container interface creation: gdbus call --system --dest dev.ovs.PortAgent1..."
echo "  • CLI interface creation: ./target/release/ovs-port-agent create-interface..."
echo "  • Introspection: sudo ./target/release/ovs-port-agent introspect"
echo ""
echo "🔄 Rollback (if needed):"
echo "  • sudo ./scripts/rollback.sh"
echo ""
echo "🎯 Ready for production use!"
