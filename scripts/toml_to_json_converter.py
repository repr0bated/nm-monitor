#!/usr/bin/env python3
"""
TOML to JSON Converter for nm-monitor Configuration

Converts the legacy TOML configuration to JSON format.
"""

import argparse
import json
import sys
from pathlib import Path

try:
    import tomllib
except ImportError:
    import tomli as tomllib


def convert_toml_to_json(toml_path: Path, json_path: Path) -> bool:
    """Convert TOML file to JSON format matching the Config struct."""

    try:
        # Read TOML file
        with open(toml_path, 'rb') as f:
            data = tomllib.load(f)

        # Convert TOML structure to JSON structure matching Config struct
        config = {}

        # Bridge configuration
        if 'bridge_name' in data:
            config['bridge'] = {
                'name': data['bridge_name'],
                'uplink': data.get('uplink'),
                'datapath_type': None,
                'fail_mode': None,
                'stp_enable': False,
                'rstp_enable': False,
                'mcast_snooping_enable': True
            }

        # NetworkManager configuration
        config['network_manager'] = {
            'interfaces_path': data.get('interfaces_path', '/etc/network/interfaces'),
            'include_prefixes': data.get('include_prefixes', ['veth', 'tap']),
            'managed_block_tag': data.get('managed_block_tag', 'ovs-port-agent'),
            'naming_template': data.get('naming_template', 'vi_{container}'),
            'enable_rename': data.get('enable_rename', True),
            'unmanaged_devices': data.get('nm_unmanaged', []),
            'connection_timeout': 45
        }

        # FUSE configuration (defaults)
        config['fuse'] = {
            'enabled': True,
            'mount_base': '/var/lib/ovs-port-agent/fuse',
            'proxmox_api_base': '/var/lib/ovs-port-agent/proxmox'
        }

        # Ledger configuration
        config['ledger'] = {
            'enabled': True,
            'path': data.get('ledger_path', '/var/lib/ovs-port-agent/ledger.jsonl'),
            'max_size_mb': 100,
            'compression_enabled': True
        }

        # Metrics configuration (defaults)
        config['metrics'] = {
            'enabled': True,
            'port': 9090,
            'path': '/metrics'
        }

        # Logging configuration (defaults)
        config['logging'] = {
            'level': 'info',
            'structured': True,
            'journald': True
        }

        # Write JSON
        with open(json_path, 'w') as f:
            json.dump(config, f, indent=2, ensure_ascii=False)
            f.write('\n')

        print(f"✅ Converted {toml_path.name} -> {json_path.name}")
        return True

    except Exception as e:
        print(f"❌ Failed to convert {toml_path.name}: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(
        description="Convert TOML configuration to JSON format"
    )
    parser.add_argument(
        "toml_file",
        help="TOML file to convert"
    )
    parser.add_argument(
        "-o", "--output",
        help="Output JSON file (default: same name with .json extension)"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing output file"
    )

    args = parser.parse_args()

    toml_path = Path(args.toml_file)
    if not toml_path.exists():
        print(f"❌ TOML file not found: {toml_path}")
        sys.exit(1)

    json_path = Path(args.output) if args.output else toml_path.with_suffix('.json')

    if json_path.exists() and not args.force:
        print(f"❌ Output file exists: {json_path} (use --force to overwrite)")
        sys.exit(1)

    success = convert_toml_to_json(toml_path, json_path)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
