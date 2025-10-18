# Building a Go Wrapper for Open vSwitch (OVS) & OVSDB via JSON-RPC

**Overview:**  
We can manage Open vSwitch directly through its OVSDB database using JSON-RPC, bypassing intermediary layers like NetworkManager or D-Bus. This involves two major components:

- A **Go client library for OVSDB** using JSON-RPC (possibly with auto-generated code from the OVSDB schema).  
- A **FUSE-based virtual filesystem** exposing OVS/OVSDB objects (tables, rows) as files/directories, with UUID-based symlinks for intuitive navigation.

## 1. Go JSON-RPC Libraries & Codegen for OVSDB

**OVSDB and JSON-RPC:** Open vSwitch’s database (OVSDB) is accessible over a JSON-RPC 2.0 protocol (RFC 7047) via a Unix socket or TCP. The schema is defined in JSON and supports introspection.

**Go Libraries:**
- **ovn-org/libovsdb**: Full-featured, supports `modelgen` to auto-generate Go structs from OVSDB schema.
- **digitalocean/go-openvswitch**: Clean Go API, includes `ovsdb` and `ovs` packages.
- **syseleven/ovsdb**: Lightweight, good for simple integrations.
- **socketplane/libovsdb**: Older, some forks used in OVN projects.

**Code Generation:** Use `get_schema` or `.ovsschema` to extract the schema, then use `modelgen` or similar tools to generate typed models for Go.

**Note:** GObject Introspection is not applicable here since OVSDB is not a GObject-based API.

## 2. Existing Go Bindings for OVSDB

- Use `libovsdb` for actively maintained and full-featured JSON-RPC client.
- It supports monitor, transactions, and model-based updates.
- `go-openvswitch` includes higher-level operations (e.g., create bridge, add port).
- For OpenFlow or kernel-level flow management, you'll need additional tooling like `ovs-ofctl` or an OpenFlow library.

## 3. FUSE Libraries in Go for Virtual Filesystem

- **hanwen/go-fuse v2**: Actively maintained, supports node-based filesystem trees, great for structured filesystems.
- **bazil.org/fuse**: Simple API, lots of tutorials, but less active.
- **billziss-gh/cgofuse**: Cross-platform but relies on C FUSE.

**Recommendation:** Use `go-fuse v2` for flexibility, maintainability, and performance.

## 4. Designing FUSE Filesystem Layout for OVSDB

**Filesystem Mapping:**
- `/ovsdb/<Table>/` → directory for each table.
- `/ovsdb/<Table>/<UUID>/` → directory for each row (use UUID).
- `/ovsdb/<Table>/<UUID>/<Column>` → file representing a column value.
- `/ovsdb/<Table>/<Name>` → symlink to the UUID dir if applicable.

**Cross-references (foreign keys):**
- Represent reference columns (e.g., Bridge.ports) as directories of symlinks to related objects.
- Implement `ReadDir` and `ReadLink` in Go FUSE to handle these.

**Writes and Transactions:**
- Writing to a file triggers a JSON-RPC `update`.
- Creating a directory or file (e.g., `mkdir`) triggers an `insert`.
- Deleting a row (e.g., `rmdir`) triggers a `delete`.

**Monitoring:** Use OVSDB `monitor` to keep the filesystem in sync with live DB changes.

## 5. Avoiding NetworkManager and D-Bus

- Direct JSON-RPC control gives full access to OVSDB features.
- You avoid abstraction limitations and translation quirks from NM/D-Bus.
- Be aware you must handle:
  - Interface bring-up
  - IP configuration
  - Conflict prevention (don’t let NM touch OVS interfaces)
  - Reconnect logic if using TCP
- FUSE needs root privileges and SELinux/AppArmor allowances.

## Summary

**Use this stack:**
- `libovsdb` or `go-openvswitch` for JSON-RPC access.
- `modelgen` to auto-generate schema structs.
- `go-fuse v2` to mount `/ovsdb` as a browsable, writable filesystem.
- Expose tables and rows as directories, columns as files.
- Use UUID symlinks to represent relationships.
- Implement transaction semantics in write/delete operations.
- Use monitor to keep FS state in sync.

This setup provides a powerful, file-driven interface to OVS networking that is decoupled from D-Bus and NetworkManager, with maximum transparency and scriptability.

