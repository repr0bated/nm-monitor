#!/usr/bin/env bash
set -euo pipefail

# NAT 1:1 mapping between public and container IPs via ovsbr0
: "${PUBLIC_IP:=80.209.242.196}"
: "${CONTAINER_IP:=10.200.0.2}"
: "${UPLINK_IF:=ovsbr0}"

sudo sysctl -w net.ipv4.ip_forward=1
sudo nft add table ip nat || true
sudo nft add chain ip nat prerouting '{ type nat hook prerouting priority -100; }' || true
sudo nft add chain ip nat postrouting '{ type nat hook postrouting priority 100; }' || true
sudo nft add rule ip nat prerouting iifname "$UPLINK_IF" ip daddr "$PUBLIC_IP" dnat to "$CONTAINER_IP" || true
sudo nft add rule ip nat postrouting ip saddr "$CONTAINER_IP" oifname "$UPLINK_IF" snat to "$PUBLIC_IP" || true
