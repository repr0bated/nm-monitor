#!/bin/bash
#
# Setup OVS Routing Layer for 80.209.242.196/25 Network
# This script configures OVS flows and routing for Xray and Warp/WireGuard
#

set -e

# Configuration
BRIDGE_NAME="ovsbr0"
CONTAINER_NETWORK="80.209.242.0/25"
CONTAINER_GATEWAY="80.209.242.1"
CONTAINER_IP="80.209.242.196"
PHYSICAL_INTERFACE="ens1"
PHYSICAL_IP="80.209.240.244"

echo "=== Setting up OVS Routing Layer for Container Network ==="

# Step 1: Configure OVS bridge with proper IP
echo "1. Configuring OVS bridge with container network IP..."
sudo ip addr add ${CONTAINER_GATEWAY}/25 dev ovsbr0if 2>/dev/null || true
sudo ip link set ovsbr0if up

# Step 2: Add physical interface to OVS bridge
echo "2. Adding physical interface to OVS bridge..."
sudo ovs-vsctl add-port ${BRIDGE_NAME} ${PHYSICAL_INTERFACE} || true

# Step 3: Configure OpenFlow rules for routing
echo "3. Configuring OpenFlow rules for container network routing..."

# Clear existing flows
sudo ovs-ofctl del-flows ${BRIDGE_NAME}

# Add routing rules
# Rule 1: Route traffic from container network to physical interface
sudo ovs-ofctl add-flow ${BRIDGE_NAME} "priority=100,in_port=ovsbr0portint,ip,nw_src=${CONTAINER_NETWORK},actions=output:${PHYSICAL_INTERFACE}"

# Rule 2: Route traffic from physical interface to container network
sudo ovs-ofctl add-flow ${BRIDGE_NAME} "priority=100,in_port=${PHYSICAL_INTERFACE},ip,nw_dst=${CONTAINER_NETWORK},actions=output:ovsbr0portint"

# Rule 3: Allow local traffic within container network
sudo ovs-ofctl add-flow ${BRIDGE_NAME} "priority=200,in_port=ovsbr0portint,ip,nw_src=${CONTAINER_NETWORK},nw_dst=${CONTAINER_NETWORK},actions=output:ovsbr0portint"

# Rule 4: Default action for other traffic
sudo ovs-ofctl add-flow ${BRIDGE_NAME} "priority=0,actions=NORMAL"

# Step 4: Configure iptables rules for NAT
echo "4. Configuring iptables NAT rules..."

# Enable IP forwarding
sudo sysctl -w net.ipv4.ip_forward=1
echo 'net.ipv4.ip_forward=1' | sudo tee -a /etc/sysctl.conf

# Configure NAT for container network
sudo iptables -t nat -A POSTROUTING -s ${CONTAINER_NETWORK} -o ${PHYSICAL_INTERFACE} -j MASQUERADE
sudo iptables -A FORWARD -i ovsbr0if -o ${PHYSICAL_INTERFACE} -j ACCEPT
sudo iptables -A FORWARD -i ${PHYSICAL_INTERFACE} -o ovsbr0if -m state --state RELATED,ESTABLISHED -j ACCEPT

# Step 5: Configure routing table
echo "5. Configuring routing table..."

# Add route for container network
sudo ip route add ${CONTAINER_NETWORK} dev ovsbr0if 2>/dev/null || true

echo "=== Container Network Routing Setup Complete ==="
echo "Container Network: ${CONTAINER_NETWORK}"
echo "Container Gateway: ${CONTAINER_GATEWAY}"
echo "Container IP: ${CONTAINER_IP}"
echo "Physical Interface: ${PHYSICAL_INTERFACE}"
echo "Physical IP: ${PHYSICAL_IP}"
