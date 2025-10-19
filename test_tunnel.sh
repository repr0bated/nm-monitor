#!/bin/bash
# Test the VPS tunnel

echo "Testing VPS tunnel on port 2222..."

if ssh -p 2222 -o ConnectTimeout=5 jeremy@localhost "echo 'Tunnel test successful!'" 2>/dev/null; then
    echo "✅ Tunnel is working!"
    echo ""
    echo "You can now run:"
    echo "  ./test_on_vps.sh test     # Test prerequisites"
    echo "  ./test_on_vps.sh install  # Run installation"
    echo "  ./test_on_vps.sh verify   # Verify success"
else
    echo "❌ Tunnel test failed"
    echo ""
    echo "Make sure the tunnel is running on your VPS:"
    echo "  ssh -i /home/jeremy/.ssh/gbjh2 jeremy@80.209.240.244"
    echo "  ./start_nm_tunnel.sh"
fi
