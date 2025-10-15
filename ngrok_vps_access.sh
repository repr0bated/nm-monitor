#!/bin/bash
# Set up ngrok tunnel for VPS access
# Alternative to SSH when ports are blocked

echo "🌐 Setting up ngrok tunnel for VPS access..."
echo ""

# Check if ngrok is installed
if ! command -v ngrok >/dev/null 2>&1; then
    echo "❌ ngrok not installed. Install from: https://ngrok.com/download"
    exit 1
fi

echo "📋 Steps:"
echo "1. On your VPS, install ngrok:"
echo "   wget https://bin.equinox.io/c/bNyj1mQVY4c/ngrok-v3-stable-linux-amd64.tgz"
echo "   tar xvf ngrok-v3-stable-linux-amd64.tgz"
echo "   sudo mv ngrok /usr/local/bin/"
echo ""
echo "2. On your VPS, start SSH tunnel:"
echo "   ngrok tcp 22"
echo ""
echo "3. ngrok will show: Forwarding tcp://0.tcp.ngrok.io:xxxxx -> localhost:22"
echo ""
echo "4. From here, connect using the ngrok URL:"
echo "   ssh jeremy@0.tcp.ngrok.io -p xxxxx"
echo ""
echo "⚠️  WARNING: This temporarily exposes SSH port to internet!"
echo "   Use only for testing, then stop ngrok."
echo ""
echo "Once connected, you can run:"
echo "scp -P xxxxx /git/nm-monitor jeremy@0.tcp.ngrok.io:/tmp/"
echo "ssh -p xxxxx jeremy@0.tcp.ngrok.io './install_ovsbr0_only.sh'"
