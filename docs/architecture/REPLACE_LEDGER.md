# Replace ledger.rs with events.jsonl

## Answer: NO, don't need ledger.rs anymore

### Old (ledger.rs):
- Central blockchain
- Plugin system
- Complex

### New (events.jsonl):
- Append-only events
- Vector DB assembles blockchain on query
- Simple

## Migration

```rust
// Remove: src/ledger.rs (623 lines)
// Add: src/events.rs (50 lines)

pub struct EventLog {
    writer: BufWriter<File>,
}

impl EventLog {
    pub fn append(&mut self, event: Event) -> Result<String> {
        let hash = hash(&event);
        writeln!(self.writer, "{}", serde_json::to_string(&event)?)?;
        self.writer.flush()?;
        Ok(hash)
    }
}
```

## Benefits
- 90% less code
- Simpler
- Faster
- Vector DB handles queries
- Btrfs handles blockchain

## Decision: Replace ledger.rs with events.jsonl
