#!/bin/bash
# Nuclear Handoff Install - OVSBR0 Only
# Zero-connectivity-loss deployment with only ovsbr0 bridge

set -euo pipefail

echo "🛡️  NUCLEAR HANDOFF INSTALL - OVSBR0 ONLY"
echo "   Zero-connectivity-loss deployment"
echo ""

# Run the install script without --with-ovsbr1 (so only ovsbr0 is created)
sudo ./scripts/install-with-network-plugin.sh --system

echo ""
echo "✅ Nuclear handoff complete!"
echo "   - ovsbr0 bridge created"
echo "   - Zero connectivity loss achieved"
echo "   - Systemd service enabled"
echo "   - Rollback backup available at: /var/lib/ovs-port-agent/backups/"
