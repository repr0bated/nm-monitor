#!/usr/bin/env bash
# Convenience wrapper to run scripts with proper sudo handling

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Function to print usage
usage() {
    cat <<EOF
Usage: $0 <script> [arguments]

Convenience wrapper to run nm-monitor scripts with proper sudo handling.

Available scripts:
  introspect        - Run introspect-network.sh
  install           - Run install-with-network-plugin.sh
  troubleshoot      - Run troubleshoot.sh
  test              - Run test-network-plugin.sh
  uninstall         - Run uninstall.sh
  rollback          - Run rollback-network.sh

Examples:
  $0 introspect
  $0 install --introspect --system
  $0 install --network-config config.yaml --with-ovsbr1 --system

This wrapper will:
  - Check if sudo is needed
  - Preserve environment variables like SUDO_USER
  - Ensure cargo is found in user's home directory
EOF
}

# Check arguments
if [[ $# -eq 0 ]] || [[ "$1" == "--help" ]] || [[ "$1" == "-h" ]]; then
    usage
    exit 0
fi

# Map short names to actual scripts
SCRIPT_NAME="$1"
shift

case "$SCRIPT_NAME" in
    introspect)
        SCRIPT_PATH="${SCRIPT_DIR}/introspect-network.sh"
        NEEDS_SUDO=true  # Recommended but not required
        ;;
    install)
        SCRIPT_PATH="${SCRIPT_DIR}/install-with-network-plugin.sh"
        NEEDS_SUDO=true  # Required
        ;;
    troubleshoot)
        SCRIPT_PATH="${SCRIPT_DIR}/troubleshoot.sh"
        NEEDS_SUDO=false
        ;;
    test)
        SCRIPT_PATH="${SCRIPT_DIR}/test-network-plugin.sh"
        NEEDS_SUDO=false
        ;;
    uninstall)
        SCRIPT_PATH="${SCRIPT_DIR}/uninstall.sh"
        NEEDS_SUDO=true
        ;;
    rollback)
        SCRIPT_PATH="${SCRIPT_DIR}/rollback-network.sh"
        NEEDS_SUDO=true
        ;;
    *)
        echo -e "${RED}ERROR: Unknown script: ${SCRIPT_NAME}${NC}" >&2
        echo ""
        usage
        exit 1
        ;;
esac

# Check if script exists
if [[ ! -f "${SCRIPT_PATH}" ]]; then
    echo -e "${RED}ERROR: Script not found: ${SCRIPT_PATH}${NC}" >&2
    exit 1
fi

# Check if we need sudo
if [[ "${NEEDS_SUDO}" == "true" ]] && [[ ${EUID} -ne 0 ]]; then
    echo -e "${YELLOW}This script requires sudo privileges.${NC}"
    echo "Running: sudo ${SCRIPT_PATH} $*"
    echo ""
    
    # Use sudo -E to preserve environment if cargo is in user's PATH
    if [[ -n "${CARGO_HOME:-}" ]] || [[ -f "$HOME/.cargo/bin/cargo" ]]; then
        exec sudo -E "${SCRIPT_PATH}" "$@"
    else
        exec sudo "${SCRIPT_PATH}" "$@"
    fi
else
    # Already root or doesn't need sudo
    exec "${SCRIPT_PATH}" "$@"
fi