#!/usr/bin/env python3
"""
YAML to JSON Converter for nm-monitor Configuration Standardization

This script converts YAML configuration files to JSON format and validates
them against the network configuration schema.
"""

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Dict, Any

import yaml
try:
    import jsonschema
    HAS_JSONSCHEMA = True
except ImportError:
    HAS_JSONSCHEMA = False
    print("Warning: jsonschema not installed. Validation will be skipped.")


class ConfigConverter:
    """Convert YAML configuration files to JSON with validation."""

    def __init__(self, schema_path: str = None):
        self.schema = None
        if schema_path and HAS_JSONSCHEMA:
            with open(schema_path, 'r') as f:
                self.schema = json.load(f)

    def load_yaml(self, yaml_path: Path) -> Dict[str, Any]:
        """Load and parse YAML file."""
        try:
            with open(yaml_path, 'r') as f:
                return yaml.safe_load(f)
        except Exception as e:
            raise ValueError(f"Failed to parse YAML {yaml_path}: {e}")

    def validate_json(self, data: Dict[str, Any], filename: str) -> None:
        """Validate JSON data against schema."""
        if not self.schema or not HAS_JSONSCHEMA:
            return

        try:
            jsonschema.validate(data, self.schema)
        except jsonschema.ValidationError as e:
            print(f"❌ Schema validation failed for {filename}:")
            print(f"   {e.message}")
            print(f"   Path: {' -> '.join(str(p) for p in e.absolute_path)}")
            raise
        except Exception as e:
            print(f"❌ Validation error for {filename}: {e}")
            raise

    def convert_file(self, yaml_path: Path, json_path: Path, validate: bool = True) -> bool:
        """Convert single YAML file to JSON."""
        try:
            # Load YAML
            data = self.load_yaml(yaml_path)

            # Validate if requested
            if validate:
                self.validate_json(data, yaml_path.name)

            # Write JSON with proper formatting
            with open(json_path, 'w') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
                f.write('\n')  # Add trailing newline

            print(f"✅ Converted {yaml_path.name} -> {json_path.name}")
            return True

        except Exception as e:
            print(f"❌ Failed to convert {yaml_path.name}: {e}")
            return False

    def convert_directory(self, source_dir: Path, target_dir: Path = None,
                         validate: bool = True) -> int:
        """Convert all YAML files in directory to JSON."""
        if target_dir is None:
            target_dir = source_dir

        target_dir.mkdir(exist_ok=True)

        converted = 0
        failed = 0

        for yaml_file in source_dir.glob("*.yaml"):
            json_file = target_dir / f"{yaml_file.stem}.json"

            if self.convert_file(yaml_file, json_file, validate):
                converted += 1
            else:
                failed += 1

        print(f"\n📊 Conversion Summary:")
        print(f"   ✅ Converted: {converted}")
        print(f"   ❌ Failed: {failed}")
        print(f"   📁 Total: {converted + failed}")

        return failed


def main():
    parser = argparse.ArgumentParser(
        description="Convert YAML configuration files to JSON format"
    )
    parser.add_argument(
        "source",
        help="Source YAML file or directory containing YAML files"
    )
    parser.add_argument(
        "-o", "--output",
        help="Output file or directory (defaults to same as source)"
    )
    parser.add_argument(
        "--schema",
        default="config/schemas/network-config.schema.json",
        help="JSON schema file for validation"
    )
    parser.add_argument(
        "--no-validate",
        action="store_true",
        help="Skip JSON schema validation"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing JSON files"
    )

    args = parser.parse_args()

    # Check if schema exists
    schema_path = Path(args.schema)
    if not args.no_validate and not schema_path.exists():
        print(f"❌ Schema file not found: {schema_path}")
        sys.exit(1)

    # Initialize converter
    converter = ConfigConverter(str(schema_path) if not args.no_validate else None)

    source_path = Path(args.source)

    if source_path.is_file():
        # Convert single file
        if not source_path.suffix == '.yaml':
            print("❌ Source must be a YAML file")
            sys.exit(1)

        output_path = Path(args.output) if args.output else source_path.with_suffix('.json')

        if output_path.exists() and not args.force:
            print(f"❌ Output file exists: {output_path} (use --force to overwrite)")
            sys.exit(1)

        success = converter.convert_file(source_path, output_path, not args.no_validate)
        sys.exit(0 if success else 1)

    elif source_path.is_dir():
        # Convert directory
        output_dir = Path(args.output) if args.output else source_path

        if output_dir.exists() and not args.force and any(output_dir.glob("*.json")):
            print(f"❌ Output directory contains JSON files: {output_dir} (use --force to overwrite)")
            sys.exit(1)

        failed = converter.convert_directory(
            source_path,
            output_dir,
            not args.no_validate
        )
        sys.exit(1 if failed > 0 else 0)

    else:
        print(f"❌ Source path not found: {source_path}")
        sys.exit(1)


if __name__ == "__main__":
    main()
