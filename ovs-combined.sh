#!/bin/bash
#
# This script uses the specific "combined" command to create the OVS bridge,
# the physical port, and the host's IP interface in separate steps,
# with the final step linking the IP interface to its port.
#
# It uses 'con-name' for maximum compatibility.

set -e # Exit immediately if a command exits with a non-zero status.

# --- Configuration ---
BRIDGE_NAME="ovsbr0"
PHYSICAL_IF="enp2s0"
IP_METHOD="auto" # "auto" for DHCP, "manual" for static

# --- Static IP Configuration (only if IP_METHOD is "manual") ---
IP_ADDRESS="172.16.0.2/24"
GATEWAY="172.16.0.1"
DNS_SERVER="8.8.8.8"

# --- Define Connection Names ---
BRIDGE_CONN="ovsbridge"
PHY_PORT_CONN="ovsport"
INT_PORT_CONN="ovsportinternal" # Port for the host IP interface
INT_IF_CONN="ovsinterface"   # The actual host IP interface

echo "### STEP 1: Cleaning up any old or conflicting connections ###"
nmcli connection delete "$BRIDGE_CONN" >/dev/null 2>&1 || true
nmcli connection delete "$PHY_PORT_CONN" >/dev/null 2>&1 || true
nmcli connection delete "$INT_PORT_CONN" >/dev/null 2>&1 || true
nmcli connection delete "$INT_IF_CONN" >/dev/null 2>&1 || true
# Clean up any lingering connections on the physical interface
for conn in $(nmcli -g NAME,DEVICE connection show | grep ":${PHYSICAL_IF}$" | cut -d: -f1); do
    echo "--> Deleting conflicting physical connection: ${conn}"
    nmcli connection delete "${conn}" >/dev/null 2>&1 || true
done

echo "### STEP 2: Creating OVS Bridge and Port Profiles ###"

# 1. Create the OVS Bridge master connection
echo "--> Creating OVS Bridge: $BRIDGE_CONN"
nmcli connection add type ovs-bridge con-name "$BRIDGE_CONN" ifname "$BRIDGE_NAME"

# 2. Create the OVS Port for the physical interface 'enp2s0'
echo "--> Creating OVS Port for $PHYSICAL_IF: $PHY_PORT_CONN"
nmcli connection add type ovs-port con-name "$PHY_PORT_CONN" ifname "$PHYSICAL_IF" master "$BRIDGE_CONN"

# 3. Create the internal OVS Port that the host's IP interface will connect to
echo "--> Creating internal OVS Port: $INT_PORT_CONN"
nmcli connection add type ovs-port con-name "$INT_PORT_CONN" ifname "$BRIDGE_NAME" master "$BRIDGE_CONN"

echo "### STEP 3: Using the 'Combined' Command to Create and Enslave the IP Interface ###"
# This is the specific command you requested.
# It creates the ovs-interface AND enslaves it to the internal port in one action.
echo "--> Creating and enslaving the host IP interface: $INT_IF_CONN"
nmcli con add type ovs-interface slave-type ovs-port con-name "$INT_IF_CONN" ifname "$BRIDGE_NAME" master "$INT_PORT_CONN"

echo "### STEP 4: Configuring the IP address on the new interface ###"
if [ "$IP_METHOD" = "manual" ]; then
    echo "--> Setting static IP: ${IP_ADDRESS}"
    nmcli connection modify "$INT_IF_CONN" ipv4.method manual ipv4.addresses "${IP_ADDRESS}" ipv4.gateway "${GATEWAY}" ipv4.dns "${DNS_SERVER}"
else
    echo "--> Setting IP via DHCP"
    nmcli connection modify "$INT_IF_CONN" ipv4.method auto
fi

echo "### STEP 5: Activating the entire configuration ###"
nmcli connection up "$BRIDGE_CONN"

echo "### ✅ Configuration Complete. Verifying status... ###"
nmcli connection show
