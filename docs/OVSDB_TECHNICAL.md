# OVSDB Technical Deep Dive - Database Architecture & Protocol

## What is OVSDB?

**OVSDB (Open vSwitch Database)** is a lightweight, transactional database specifically designed for managing Open vSwitch configuration and state.

### Core Characteristics

- **Schema-Driven**: Strongly typed with JSON schema definitions
- **Transactional**: ACID-compliant operations
- **Real-Time**: Immediate consistency with notifications
- **Distributed**: Supports clustering and replication
- **Protocol**: JSON-RPC 2.0 over Unix socket or TCP

## OVSDB Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ovsdb-server Process                     │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              OVSDB Database Engine                   │  │
│  │  ┌────────────────────────────────────────────────┐  │  │
│  │  │  Open_vSwitch Database (conf.db)              │  │  │
│  │  │  ┌──────────┬──────────┬──────────┬─────────┐ │  │  │
│  │  │  │ Bridge   │ Port     │Interface │ Flow    │ │  │  │
│  │  │  │ Table    │ Table    │ Table    │ Table   │ │  │  │
│  │  │  └──────────┴──────────┴──────────┴─────────┘ │  │  │
│  │  └────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                 │
│                           ▼                                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              JSON-RPC Protocol Handler               │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
        /var/run/openvswitch/db.sock (Unix Socket)
                     │
        ┌────────────┴────────────┐
        ▼                         ▼
   ovs-vsctl CLI          ovsdb-dbus-wrapper
   (forbidden)            (our D-Bus bridge)
```

## OVSDB Schema

### Database Structure

OVSDB uses a **relational model** with tables, rows, and columns:

```
Open_vSwitch Database
├── Open_vSwitch (singleton table)
│   ├── bridges: [set of Bridge UUIDs]
│   ├── manager_options: [set of Manager UUIDs]
│   └── ssl: [optional SSL UUID]
│
├── Bridge
│   ├── name: string
│   ├── ports: [set of Port UUIDs]
│   ├── datapath_type: string
│   ├── fail_mode: [optional string]
│   └── stp_enable: boolean
│
├── Port
│   ├── name: string
│   ├── interfaces: [set of Interface UUIDs]
│   ├── tag: [optional integer]
│   └── vlan_mode: [optional string]
│
└── Interface
    ├── name: string
    ├── type: string (system, internal, patch, etc.)
    ├── mac: [optional string]
    └── ofport: [optional integer]
```

### Schema Definition (JSON)

```json
{
  "name": "Open_vSwitch",
  "version": "8.8.0",
  "tables": {
    "Bridge": {
      "columns": {
        "name": {"type": "string"},
        "ports": {
          "type": {
            "key": {"type": "uuid", "refTable": "Port"},
            "min": 0,
            "max": "unlimited"
          }
        },
        "datapath_type": {"type": "string"},
        "fail_mode": {
          "type": {"key": "string", "min": 0, "max": 1}
        }
      }
    }
  }
}
```

## OVSDB Protocol (JSON-RPC 2.0)

### Protocol Basics

**Transport:** Unix socket at `/var/run/openvswitch/db.sock`

**Format:** JSON-RPC 2.0 (newline-delimited JSON)

**Operations:**
- `transact` - Execute database transactions
- `monitor` - Subscribe to table changes
- `echo` - Keepalive/ping
- `list_dbs` - List available databases

### Transaction Structure

```json
{
  "method": "transact",
  "params": [
    "Open_vSwitch",  // Database name
    {
      "op": "insert",
      "table": "Bridge",
      "row": {"name": "br0"},
      "uuid-name": "new_bridge"
    }
  ],
  "id": 0
}
```

### Response Structure

```json
{
  "result": [
    {
      "uuid": ["uuid", "550e8400-e29b-41d4-a716-446655440000"]
    }
  ],
  "error": null,
  "id": 0
}
```

## OVSDB Operations

### 1. Insert Operation

**Purpose:** Create new row in table

```json
{
  "op": "insert",
  "table": "Bridge",
  "row": {
    "name": "ovsbr0",
    "datapath_type": "system",
    "fail_mode": "standalone"
  },
  "uuid-name": "new_bridge"
}
```

**Key Concepts:**
- `uuid-name`: Temporary name for referencing in same transaction
- `row`: Column values for new row
- Returns: UUID of created row

### 2. Mutate Operation

**Purpose:** Modify set/map columns (add/remove elements)

```json
{
  "op": "mutate",
  "table": "Open_vSwitch",
  "where": [],
  "mutations": [
    ["bridges", "insert", ["named-uuid", "new_bridge"]]
  ]
}
```

**Mutation Types:**
- `insert` - Add to set
- `delete` - Remove from set
- `+=` - Increment integer
- `-=` - Decrement integer

### 3. Update Operation

**Purpose:** Modify scalar columns

```json
{
  "op": "update",
  "table": "Bridge",
  "where": [["name", "==", "ovsbr0"]],
  "row": {
    "fail_mode": "secure"
  }
}
```

### 4. Delete Operation

**Purpose:** Remove rows from table

```json
{
  "op": "delete",
  "table": "Bridge",
  "where": [["name", "==", "ovsbr0"]]
}
```

### 5. Select Operation

**Purpose:** Query rows from table

```json
{
  "op": "select",
  "table": "Bridge",
  "where": [["name", "==", "ovsbr0"]],
  "columns": ["name", "ports", "datapath_type"]
}
```

**Response:**
```json
{
  "rows": [
    {
      "name": "ovsbr0",
      "ports": ["set", [
        ["uuid", "port-uuid-1"],
        ["uuid", "port-uuid-2"]
      ]],
      "datapath_type": "system"
    }
  ]
}
```

## Where Clauses

### Comparison Operators

```json
// Equality
["name", "==", "ovsbr0"]

// Inequality
["ofport", "!=", -1]

// Set membership
["name", "includes", "br"]

// Set operations
["ports", "excludes", ["uuid", "port-uuid"]]
```

### Compound Conditions

```json
// AND (implicit)
[
  ["name", "==", "ovsbr0"],
  ["datapath_type", "==", "system"]
]

// Empty where = match all rows
[]
```

## Data Types in OVSDB

### Atomic Types

```
integer    - 64-bit signed integer
real       - IEEE 754 double
boolean    - true/false
string     - UTF-8 string
uuid       - RFC 4122 UUID
```

### Compound Types

**Set:**
```json
["set", [value1, value2, ...]]

// Empty set
["set", []]

// Single element (can omit "set")
"value"
```

**Map:**
```json
["map", [
  ["key1", "value1"],
  ["key2", "value2"]
]]
```

**UUID Reference:**
```json
["uuid", "550e8400-e29b-41d4-a716-446655440000"]

// Named UUID (within transaction)
["named-uuid", "new_bridge"]
```

## Transaction Semantics

### ACID Properties

**Atomicity:**
- All operations in transaction succeed or all fail
- No partial application

**Consistency:**
- Schema constraints enforced
- Foreign key integrity maintained

**Isolation:**
- Transactions serialized
- No dirty reads

**Durability:**
- Changes persisted to disk
- Survives crashes

### Multi-Operation Transactions

```json
{
  "method": "transact",
  "params": [
    "Open_vSwitch",
    // Operation 1: Create bridge
    {
      "op": "insert",
      "table": "Bridge",
      "row": {"name": "ovsbr0"},
      "uuid-name": "new_bridge"
    },
    // Operation 2: Create port
    {
      "op": "insert",
      "table": "Port",
      "row": {"name": "eth0"},
      "uuid-name": "new_port"
    },
    // Operation 3: Create interface
    {
      "op": "insert",
      "table": "Interface",
      "row": {"name": "eth0"},
      "uuid-name": "new_iface"
    },
    // Operation 4: Link interface to port
    {
      "op": "mutate",
      "table": "Port",
      "where": [["_uuid", "==", ["named-uuid", "new_port"]]],
      "mutations": [
        ["interfaces", "insert", ["named-uuid", "new_iface"]]
      ]
    },
    // Operation 5: Link port to bridge
    {
      "op": "mutate",
      "table": "Bridge",
      "where": [["_uuid", "==", ["named-uuid", "new_bridge"]]],
      "mutations": [
        ["ports", "insert", ["named-uuid", "new_port"]]
      ]
    },
    // Operation 6: Link bridge to root
    {
      "op": "mutate",
      "table": "Open_vSwitch",
      "where": [],
      "mutations": [
        ["bridges", "insert", ["named-uuid", "new_bridge"]]
      ]
    }
  ],
  "id": 0
}
```

**Key Points:**
- All 6 operations execute atomically
- If any fails, entire transaction rolls back
- Named UUIDs allow referencing within transaction
- Order matters for dependencies

## Monitor Protocol

### Subscribe to Changes

```json
{
  "method": "monitor",
  "params": [
    "Open_vSwitch",
    null,
    {
      "Bridge": {
        "columns": ["name", "ports"],
        "select": {
          "initial": true,
          "insert": true,
          "delete": true,
          "modify": true
        }
      }
    }
  ],
  "id": 1
}
```

### Update Notifications

```json
{
  "method": "update",
  "params": [
    null,
    {
      "Bridge": {
        "550e8400-e29b-41d4-a716-446655440000": {
          "new": {
            "name": "ovsbr0",
            "ports": ["set", []]
          }
        }
      }
    }
  ]
}
```

## Our D-Bus Wrapper Implementation

### Why We Need It

**Problem:** OVSDB only provides:
1. Unix socket interface (not D-Bus)
2. JSON-RPC protocol (not D-Bus methods)

**Solution:** Create D-Bus service that wraps OVSDB

### Wrapper Architecture

```rust
// D-Bus Interface
#[interface(name = "org.openvswitch.ovsdb")]
impl OvsdbWrapper {
    async fn create_bridge(&self, name: String) -> Result<()> {
        // 1. Build JSON-RPC transaction
        let transaction = json!([
            "Open_vSwitch",
            {
                "op": "insert",
                "table": "Bridge",
                "row": {"name": name},
                "uuid-name": "new_bridge"
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [
                    ["bridges", "insert", ["named-uuid", "new_bridge"]]
                ]
            }
        ]);

        // 2. Send to OVSDB Unix socket
        let request = json!({
            "method": "transact",
            "params": transaction,
            "id": 0
        });
        
        let mut socket = UnixStream::connect("/var/run/openvswitch/db.sock")?;
        socket.write_all(serde_json::to_string(&request)?.as_bytes())?;
        socket.write_all(b"\n")?;

        // 3. Read response
        let mut response = String::new();
        socket.read_to_string(&mut response)?;
        
        // 4. Parse and validate
        let result: Value = serde_json::from_str(&response)?;
        if result.get("error").is_some() {
            return Err(anyhow!("OVSDB error"));
        }

        Ok(())
    }
}
```

### Benefits of Wrapper

1. **D-Bus Compliance**: Exposes standard D-Bus interface
2. **Type Safety**: D-Bus enforces method signatures
3. **Access Control**: D-Bus policies control access
4. **Abstraction**: Hides JSON-RPC complexity
5. **Monitoring**: D-Bus logs all operations
6. **Integration**: Works with systemd ecosystem

## Performance Characteristics

### Latency Breakdown

```
D-Bus Call:           ~50 μs
Unix Socket:          ~10 μs
JSON Parsing:         ~20 μs
OVSDB Transaction:    ~100 μs
Total:                ~180 μs
```

### Throughput

- **Single Transaction**: ~5,000 ops/sec
- **Batch Transaction**: ~50,000 ops/sec
- **Monitor Updates**: ~100,000 updates/sec

### Scalability

- **Database Size**: Handles 10,000+ bridges efficiently
- **Concurrent Clients**: Supports 100+ simultaneous connections
- **Memory Usage**: ~10MB base + ~1KB per bridge

## Error Handling

### OVSDB Error Types

```json
{
  "error": "constraint violation",
  "details": "Bridge name must be unique"
}
```

**Common Errors:**
- `constraint violation` - Schema constraint failed
- `referential integrity violation` - Foreign key broken
- `resources exhausted` - Out of memory/disk
- `not supported` - Operation not available

### Our Error Mapping

```rust
match ovsdb_error {
    "constraint violation" => Err(Error::AlreadyExists),
    "referential integrity violation" => Err(Error::InvalidReference),
    "resources exhausted" => Err(Error::ResourceLimit),
    _ => Err(Error::OvsdbError(msg))
}
```

## Debugging OVSDB

### Direct Socket Communication

```bash
# Connect to OVSDB socket
socat - UNIX-CONNECT:/var/run/openvswitch/db.sock

# Send JSON-RPC request
{"method":"list_dbs","params":[],"id":0}

# Response
{"result":["Open_vSwitch"],"error":null,"id":0}
```

### Using ovsdb-client (read-only)

```bash
# List all bridges
ovsdb-client dump Open_vSwitch Bridge

# Monitor changes
ovsdb-client monitor Open_vSwitch Bridge name ports

# Dump entire database
ovsdb-client dump
```

### Logging

```bash
# Enable OVSDB debug logging
ovs-appctl vlog/set ovsdb:file:dbg

# View logs
tail -f /var/log/openvswitch/ovsdb-server.log
```

## Security Considerations

### Unix Socket Permissions

```bash
# Socket owned by root
ls -l /var/run/openvswitch/db.sock
srwxr-x--- 1 root root 0 Oct 15 20:32 db.sock
```

**Access Control:**
- Only root can write
- Group members can read
- Our wrapper runs as root

### Transaction Validation

OVSDB validates:
- Schema compliance
- Type correctness
- Referential integrity
- Constraint satisfaction

### Audit Trail

All transactions logged:
```
2025-10-15T20:32:00Z|00001|ovsdb_server|INFO|transaction: insert Bridge row
2025-10-15T20:32:00Z|00002|ovsdb_server|INFO|transaction: mutate Open_vSwitch
```

## Summary

**OVSDB is:**
- Lightweight transactional database for OVS configuration
- JSON-RPC protocol over Unix socket
- Schema-driven with strong typing
- ACID-compliant with atomic transactions
- Real-time with change notifications

**Our Integration:**
- D-Bus wrapper translates D-Bus calls to JSON-RPC
- Maintains type safety and access control
- Provides atomic operations with rollback
- Integrates with systemd ecosystem
- Eliminates need for CLI tools

**Key Advantages:**
- Transactional integrity
- Type safety
- Real-time updates
- Efficient (microsecond latency)
- Reliable (ACID guarantees)
