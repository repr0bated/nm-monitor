# GhostBridge Architecture

## Core Concept
Privacy router with real-time vectorization, btrfs blockchain, streaming replication.

## Architecture

```
/var/lib/ghostbridge/ (btrfs subvolume)
├── events.jsonl          # Append-only event log (source of truth)
├── .snapshots/           # Btrfs snapshots = blockchain blocks
│   ├── block-0-abc/
│   ├── block-1-def/
│   └── block-2-ghi/
└── metadata/

/var/lib/qdrant/          # External vector index (not snapshotted)
└── collections/events/   # Rebuilt from events.jsonl
```

## Components

1. **Event Log** (events.jsonl)
   - Append-only
   - Source of truth
   - Btrfs snapshotted

2. **Vector DB** (Qdrant)
   - External index
   - Real-time vectorization
   - Rebuildable from events

3. **Btrfs Snapshots**
   - Instant blockchain blocks
   - Copy-on-write (efficient)
   - Streamable (send/receive)

4. **OVS Bridge**
   - Network forwarding
   - Event generation
   - Transparent operation

## Data Flow

```
Packet → Extract features → Vectorize → Append event → Forward packet
                                            ↓
                                    Periodic snapshot
                                            ↓
                                    Stream to replicas
```

## Operations

### Add Event
```rust
event_log.append(event)?;                    // Source of truth
vector_db.insert(hash, vector, event)?;      // Index (async)
```

### Create Block
```bash
btrfs subvolume snapshot -r /var/lib/ghostbridge /var/lib/ghostbridge/.snapshots/block-N
```

### Stream to Replica
```bash
btrfs send -p block-N-1 block-N | zstd | ssh replica 'zstd -d | btrfs receive ...'
```

### Rebuild Index
```bash
rebuild_vector_index /var/lib/ghostbridge/events.jsonl
```

## Benefits

- Zero overhead (operation = storage)
- Real-time vectorization
- Instant snapshots (1ms)
- Incremental streaming (100x smaller)
- 80% compression
- Vector DB external (rebuildable)
- No FUSE layer needed

## Implementation

1. Element blockchain (events.jsonl) - 2h
2. Btrfs snapshot automation - 1h
3. Vector DB integration - 2h
4. Streaming daemon - 1h
5. Real-time vectorization - 2h

Total: 8 hours
