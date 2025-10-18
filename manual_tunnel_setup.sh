#!/bin/bash
# Manual tunnel setup for nm-monitor VPS testing

VPS_IP="80.209.240.244"
VPS_USER="jeremy"
LOCAL_USER="jeremy"
SSH_KEY="/home/jeremy/.ssh/gbjh2"
TUNNEL_PORT="2222"

echo "Setting up SSH tunnel to VPS..."
echo "VPS: $VPS_USER@$VPS_IP"
echo "Local: $LOCAL_USER@localhost:$TUNNEL_PORT"
echo ""

# Test connection
echo "Testing SSH connection..."
if ssh -i "$SSH_KEY" -o ConnectTimeout=10 "$VPS_USER@$VPS_IP" "echo 'SSH connection successful'"; then
    echo "✅ SSH connection works!"
else
    echo "❌ SSH connection failed"
    exit 1
fi

echo ""
echo "Creating tunnel script on VPS..."

# Create tunnel script on VPS
ssh -i "$SSH_KEY" "$VPS_USER@$VPS_IP" << REMOTE_EOF
cat > ~/start_nm_tunnel.sh << LOCAL_EOF
#!/bin/bash
# nm-monitor VPS testing tunnel
echo "Starting nm-monitor testing tunnel..."
echo "Press Ctrl+C to stop"

# Keep tunnel alive
while true; do
    echo "[$(date)] Connecting tunnel..."
    ssh -i /home/jeremy/.ssh/gbjh2 -o StrictHostKeyChecking=no -o ServerAliveInterval=30 -R ${TUNNEL_PORT}:localhost:22 jeremy@$VPS_IP
    echo "[$(date)] Tunnel disconnected, retrying in 5 seconds..."
    sleep 5
done
LOCAL_EOF

chmod +x ~/start_nm_tunnel.sh
echo ""
echo "✅ Tunnel script created on VPS!"
echo "Run this on your VPS: ./start_nm_tunnel.sh"
echo ""
echo "Then test from your local machine:"
echo "ssh -p $TUNNEL_PORT $VPS_USER@localhost"
REMOTE_EOF

echo ""
echo "🎉 Setup complete!"
echo ""
echo "NEXT STEPS:"
echo "1. SSH to your VPS: ssh -i $SSH_KEY $VPS_USER@$VPS_IP"
echo "2. On VPS, run: ./start_nm_tunnel.sh"
echo "3. Back locally, test: ssh -p $TUNNEL_PORT $VPS_USER@localhost"
echo "4. Then run: ./test_on_vps.sh test"
