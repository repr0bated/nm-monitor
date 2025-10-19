---
description: Repository Information Overview
alwaysApply: true
---

# OVS Port Agent Information

## Summary
OVS Port Agent (nm-monitor) is a Rust-based network management system that provides zero-connectivity-loss OVS bridge management through atomic handover techniques, D-Bus introspection, and blockchain ledger for network operation accountability. It integrates with NetworkManager, Proxmox, and Netmaker for comprehensive network management.

## Structure
- **src/**: Core Rust source code with main application logic
- **systemd/**: Service files for system integration
- **config/**: Configuration templates and examples
- **scripts/**: Installation and management scripts
- **dbus/**: D-Bus configuration files
- **docs/**: Comprehensive documentation and architecture guides
- **tests/**: Test files for functionality verification

## Language & Runtime
**Language**: Rust
**Version**: Edition 2021
**Build System**: Cargo
**Package Manager**: Cargo

## Dependencies
**Main Dependencies**:
- tokio (1.x): Async runtime with multi-threading support
- zbus (4.x): D-Bus communication
- serde/serde_json (1.x): Serialization/deserialization
- rtnetlink (0.14): Network interface management
- fuser (0.14): FUSE filesystem implementation
- clap (4.x): Command-line argument parsing

**Development Dependencies**:
- systemd-journal-logger (2.x)
- tracing/tracing-subscriber
- tempfile (3.0)
- validator (0.20.0)

## Build & Installation
```bash
# Build the project
cargo build --release

# Install with basic configuration
sudo ./scripts/install.sh --bridge vmbr0 --uplink enp2s0 --system
```

## Docker
**Dockerfile**: Dockerfile (multi-stage build)
**Base Images**: rust:1.75-slim (builder), debian:bookworm-slim (runtime)
**Configuration**: Includes OpenVSwitch, systemd, and D-Bus dependencies
**Exposed Ports**: 8080

## Testing
**Framework**: Rust's built-in testing framework
**Test Location**: tests/ directory and inline module tests
**Naming Convention**: *_test.rs files
**Run Command**:
```bash
cargo test
```

## Main Components
- **ovs-port-agent**: Main service for OVS bridge management
- **ovsdb-dbus-wrapper**: D-Bus wrapper for OVSDB communication
- **ovsdb-fuse-mount**: FUSE filesystem for enhanced Proxmox integration
- **system-introspection**: System state discovery service

## Key Features
- **Blockchain Ledger**: SHA-256 hash chain for network operation accountability
- **D-Bus API**: Comprehensive system introspection and control
- **Atomic Operations**: Zero-connectivity-loss network changes
- **Proxmox Integration**: Enhanced GUI compatibility and VMID mapping
- **Netmaker Support**: Container auto-detection and mesh networking