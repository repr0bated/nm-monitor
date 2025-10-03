#!/usr/bin/env bash
set -euo pipefail

# Add a secondary public IP to the ovsbr0 NM connection
: "${CONN:=ovsbr0}"
: "${IPADDR:=80.209.242.196/25}"

sudo nmcli c modify "$CONN" +ipv4.addresses "$IPADDR"
sudo nmcli c up "$CONN"