#!/usr/bin/env bash
# Troubleshooting agent for nm-monitor installation

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "=========================================="
echo " nm-monitor Troubleshooting Agent"
echo "=========================================="
echo ""

# Check if running as root
if [[ $EUID -eq 0 ]]; then
    echo -e "${GREEN}✓${NC} Running as root"
else
    echo -e "${YELLOW}!${NC} Not running as root (some checks may fail)"
fi
echo ""

echo "=========================================="
echo " 1. Network Interface Detection"
echo "=========================================="
echo ""

echo "All interfaces:"
ip -brief addr show
echo ""

echo "IPv4 interfaces:"
ip -o -4 addr show
echo ""

echo "Physical interfaces (after filtering):"
ip -o -4 addr show | grep -v "lo" | grep -v "ovsbr" | grep -v "docker" | grep -v "br-" | grep -v "veth"
echo ""

PRIMARY_IFACE=$(ip -o -4 addr show | grep -v "lo" | grep -v "ovsbr" | grep -v "docker" | grep -v "br-" | grep -v "veth" | head -1 | awk '{print $2}' || echo "")

if [[ -n "${PRIMARY_IFACE}" ]]; then
    echo -e "${GREEN}✓${NC} Primary interface detected: ${PRIMARY_IFACE}"
    
    IP_ADDR=$(ip -o -4 addr show "${PRIMARY_IFACE}" | awk '{print $4}' | cut -d/ -f1)
    PREFIX=$(ip -o -4 addr show "${PRIMARY_IFACE}" | awk '{print $4}' | cut -d/ -f2)
    GATEWAY=$(ip route show default | grep "${PRIMARY_IFACE}" | awk '{print $3}' | head -1)
    
    echo "  IP: ${IP_ADDR}/${PREFIX}"
    echo "  Gateway: ${GATEWAY:-none}"
else
    echo -e "${RED}✗${NC} No primary interface detected!"
    echo "  This will cause introspection to fail"
fi
echo ""

echo "=========================================="
echo " 2. OVS Status"
echo "=========================================="
echo ""

if command -v ovs-vsctl >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} ovs-vsctl found"
    
    if systemctl is-active --quiet openvswitch-switch || systemctl is-active --quiet ovs-vswitchd; then
        echo -e "${GREEN}✓${NC} OVS service running"
        
        if sudo ovs-vsctl show >/dev/null 2>&1; then
            echo -e "${GREEN}✓${NC} OVS database accessible"
            echo ""
            echo "Current OVS config:"
            sudo ovs-vsctl show
        else
            echo -e "${RED}✗${NC} Cannot access OVS database"
        fi
    else
        echo -e "${RED}✗${NC} OVS service not running"
    fi
else
    echo -e "${RED}✗${NC} ovs-vsctl not found"
fi
echo ""

echo "=========================================="
echo " 3. Cargo/Rust Status"
echo "=========================================="
echo ""

if command -v cargo >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Cargo found: $(command -v cargo)"
    cargo --version
elif [[ -f "${HOME}/.cargo/bin/cargo" ]]; then
    echo -e "${GREEN}✓${NC} Cargo found in user home: ${HOME}/.cargo/bin/cargo"
    "${HOME}/.cargo/bin/cargo" --version
elif [[ -n "${SUDO_USER:-}" ]] && [[ -f "/home/${SUDO_USER}/.cargo/bin/cargo" ]]; then
    echo -e "${GREEN}✓${NC} Cargo found for sudo user: /home/${SUDO_USER}/.cargo/bin/cargo"
    sudo -u "${SUDO_USER}" /home/${SUDO_USER}/.cargo/bin/cargo --version
else
    echo -e "${RED}✗${NC} Cargo not found"
    echo "  Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi
echo ""

echo "=========================================="
echo " 4. Python Status"
echo "=========================================="
echo ""

if command -v python3 >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Python3 found: $(python3 --version)"
    
    if python3 -c "import yaml" 2>/dev/null; then
        echo -e "${GREEN}✓${NC} PyYAML installed"
    else
        echo -e "${RED}✗${NC} PyYAML not installed"
        echo "  Install: sudo apt install python3-yaml"
    fi
else
    echo -e "${RED}✗${NC} Python3 not found"
fi
echo ""

echo "=========================================="
echo " 5. Systemd Status"
echo "=========================================="
echo ""

if command -v systemctl >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} systemd found"
    
    if systemctl is-active --quiet systemd-networkd; then
        echo -e "${GREEN}✓${NC} systemd-networkd running"
    else
        echo -e "${YELLOW}!${NC} systemd-networkd not running"
    fi
    
    if systemctl is-active --quiet dbus; then
        echo -e "${GREEN}✓${NC} D-Bus running"
    else
        echo -e "${RED}✗${NC} D-Bus not running"
    fi
else
    echo -e "${RED}✗${NC} systemctl not found"
fi
echo ""

echo "=========================================="
echo " 6. Existing Installation"
echo "=========================================="
echo ""

if [[ -f /usr/local/bin/ovs-port-agent ]]; then
    echo -e "${GREEN}✓${NC} Binary installed: /usr/local/bin/ovs-port-agent"
    /usr/local/bin/ovs-port-agent --version 2>/dev/null || echo "  (no version info)"
else
    echo -e "${YELLOW}!${NC} Binary not installed"
fi

if [[ -f /etc/ovs-port-agent/config.toml ]]; then
    echo -e "${GREEN}✓${NC} Config exists: /etc/ovs-port-agent/config.toml"
else
    echo -e "${YELLOW}!${NC} Config not found"
fi

if [[ -d /var/lib/ovs-port-agent ]]; then
    echo -e "${GREEN}✓${NC} Ledger directory exists"
    ls -lah /var/lib/ovs-port-agent/ 2>/dev/null | head -5 || true
else
    echo -e "${YELLOW}!${NC} Ledger directory not found"
fi

if systemctl is-active --quiet ovs-port-agent; then
    echo -e "${GREEN}✓${NC} Service running"
    systemctl status --no-pager ovs-port-agent | head -10
else
    echo -e "${YELLOW}!${NC} Service not running"
fi
echo ""

echo "=========================================="
echo " 7. Test Introspection Manually"
echo "=========================================="
echo ""

if [[ -f ./scripts/introspect-network.sh ]]; then
    echo "Running introspection test..."
    ./scripts/introspect-network.sh /tmp/troubleshoot-introspect.yaml vmbr0 2>&1 || echo -e "${RED}Introspection failed${NC}"
    
    if [[ -f /tmp/troubleshoot-introspect.yaml ]]; then
        echo -e "${GREEN}✓${NC} Introspection created file"
        echo ""
        echo "Generated config:"
        cat /tmp/troubleshoot-introspect.yaml
        rm -f /tmp/troubleshoot-introspect.yaml
    else
        echo -e "${RED}✗${NC} No file created"
    fi
else
    echo -e "${RED}✗${NC} Introspection script not found"
fi
echo ""

echo "=========================================="
echo " Summary & Recommendations"
echo "=========================================="
echo ""

# Generate recommendations
ISSUES=0

if [[ -z "${PRIMARY_IFACE:-}" ]]; then
    echo -e "${RED}CRITICAL:${NC} No primary interface detected"
    echo "  → Check network configuration with: ip addr show"
    ISSUES=$((ISSUES + 1))
fi

if ! command -v ovs-vsctl >/dev/null 2>&1; then
    echo -e "${RED}CRITICAL:${NC} OVS not installed"
    echo "  → Install: sudo apt install openvswitch-switch"
    ISSUES=$((ISSUES + 1))
fi

if ! command -v cargo >/dev/null 2>&1 && [[ ! -f "${HOME}/.cargo/bin/cargo" ]]; then
    echo -e "${RED}CRITICAL:${NC} Rust/Cargo not installed"
    echo "  → Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    ISSUES=$((ISSUES + 1))
fi

if ! python3 -c "import yaml" 2>/dev/null; then
    echo -e "${YELLOW}WARNING:${NC} PyYAML not installed"
    echo "  → Install: sudo apt install python3-yaml"
    ISSUES=$((ISSUES + 1))
fi

if [[ ${ISSUES} -eq 0 ]]; then
    echo -e "${GREEN}✓ All prerequisites met!${NC}"
    echo ""
    echo "Ready to install. Run:"
    echo "  sudo ./scripts/install-with-network-plugin.sh --introspect --with-ovsbr1 --system"
else
    echo ""
    echo -e "${RED}Found ${ISSUES} issue(s) that need attention${NC}"
fi

echo ""
echo "=========================================="
echo " Troubleshooting Complete"
echo "=========================================="

