bash
  set -euo pipefail

  # 1) Add gratuitous ARP in fallback path (only if not already present)

  if ! grep -q 'arping -U -I ovsbr0' scripts/install-with-network-plugin.sh; then
  SNIP="$(mktemp)"
  cat > "${SNIP}" << 'EOF'

  # Send gratuitous ARP to refresh neighbor caches (minimize interruption)

  if command -v arping >/dev/null 2>&1; then
  log "Sending gratuitous ARP for ${UPLINK_IP} on ovsbr0..."
  arping -U -I ovsbr0 -c 3 "${UPLINK_IP}" >/dev/null 2>&1 || true
  if [[ -n "${UPLINK_GW}" ]]; then
  arping -S "${UPLINK_IP}" -I ovsbr0 -c 2 "${UPLINK_GW}" >/dev/null 2>&1 || true
  fi
  fi
  EOF

  # Insert after the default route switch to ovsbr0 (first occurrence)

  sed -i "/ip route add default via .*dev ovsbr0 || true/ r ${SNIP}" scripts/install-with-network-plugin.sh
  rm -f "${SNIP}"
  fi

  # 2) Create codex branch, commit all current changes

  git checkout -b codex
  git add -A
  git commit -m "codex: zbus networkd wrapper, native OVS detection + ovs-vsctl fallback, zbus-net CLI, D-Bus Reload,
  atomic .network writes, Kind=ovs-bridge fixes, gratuitous ARP in fallback"

  # 3) Merge remote cursor branches (fetch first)

  git fetch --all --prune

  # Merge without editing message; if conflicts arise, resolve then git add/commit as usual

  git merge --no-edit origin/cursor/debug-install-script-1863 || true
  git merge --no-edit origin/cursor/debug-netlink-rust-code-0838 || true

  # 4) Show result

  echo; git status -uno
  echo; git branch -vv

  # 5) Optional: build to verify it’s clean

  # cargo build
