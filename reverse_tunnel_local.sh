#!/bin/bash
# Create reverse tunnel from local machine to VPS
# Allows VPS to connect back to local machine

echo "🔄 Setting up reverse tunnel from local to VPS..."
echo ""

# Check if we have VPS IP
if [ -z "${VPS_IP:-}" ]; then
    echo "❌ Set VPS_IP environment variable first:"
    echo "   export VPS_IP=80.209.240.244"
    exit 1
fi

echo "📋 Reverse Tunnel Setup:"
echo ""
echo "This will create a tunnel FROM your local machine TO the VPS"
echo "The VPS can then connect back to your local machine on port 2222"
echo ""
echo "Run this on your local machine:"
echo "ssh -R 2222:localhost:22 jeremy@$VPS_IP"
echo ""
echo "Then on VPS, you can connect back:"
echo "ssh jeremy@localhost -p 2222"
echo ""
echo "Once connected, run the install:"
echo "scp /git/nm-monitor jeremy@localhost:/tmp/"
echo "ssh jeremy@localhost './install_ovsbr0_only.sh'"
echo ""
echo "🎯 This works even behind Verizon modem!"
