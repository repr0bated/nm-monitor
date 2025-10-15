# Property Accountability Model

## Philosophy

Properties follow an **append-only** model for accountability and audit trail integrity.

## Rules

### 1. Field Names (Keys) - APPEND-ONLY ✓

- Once a property field is added, it **persists forever**
- Cannot be deleted or removed
- Introspection automatically adds fields for discovered hardware properties
- Users can add custom fields
- All field names tracked in `property_schema` array

### 2. Field Values - MUTABLE ✓

- Values CAN be changed
- All value changes logged to blockchain ledger
- Provides full audit trail of all modifications

### 3. Validation

- Plugin validates that all fields in `property_schema` exist in `properties`
- Attempting to remove a field triggers validation error
- New fields can be added freely

## Example Flow

### Initial Introspection

```yaml
properties:
  mac_address: "aa:bb:cc:dd:ee:ff"
  mtu: 1500
  speed: 1000

property_schema:
  - mac_address
  - mtu
  - speed
```

### User Adds Custom Field

```yaml
properties:
  mac_address: "aa:bb:cc:dd:ee:ff"
  mtu: 1500
  speed: 1000
  datacenter: "dc1"          # NEW

property_schema:
  - mac_address
  - mtu
  - speed
  - datacenter               # ADDED
```

### Hardware Changes (Valid)

```yaml
properties:
  mac_address: "11:22:33:44:55:66"  # VALUE CHANGED ✓
  mtu: 9000                          # VALUE CHANGED ✓
  speed: 10000                       # VALUE CHANGED ✓
  datacenter: "dc2"                  # VALUE CHANGED ✓

property_schema:
  - mac_address                      # FIELD STILL PRESENT
  - mtu                              # FIELD STILL PRESENT
  - speed                            # FIELD STILL PRESENT
  - datacenter                       # FIELD STILL PRESENT
```

Ledger shows:
```
Block 1: mac_address: "aa:bb:cc:dd:ee:ff" → "11:22:33:44:55:66"
Block 2: mtu: 1500 → 9000
Block 3: speed: 1000 → 10000
Block 4: datacenter: "dc1" → "dc2"
```

### Invalid: Removing Field ✗

```yaml
properties:
  mac_address: "11:22:33:44:55:66"
  mtu: 9000
  # speed REMOVED ✗

property_schema:
  - mac_address
  - mtu
  - speed        # Still in schema!
  - datacenter
```

**ERROR**: Property 'speed' declared in schema but missing from properties (append-only violation)

## Benefits

1. **Accountability**: Full history of what was discovered and when
2. **Audit Trail**: Ledger shows all value changes
3. **Extensibility**: Users can add custom fields for unforeseen hardware
4. **Immutability**: Field names never disappear, preventing data loss
5. **Flexibility**: Values can update as hardware changes

## Use Cases

### Multiple Email Addresses

```yaml
properties:
  admin_email: "admin@example.com"
  ops_email: "ops@example.com"
  oncall_email: "oncall@example.com"
  
property_schema:
  - admin_email
  - ops_email
  - oncall_email
```

### Multiple MAC Addresses (Hardware with Virtual Functions)

```yaml
properties:
  mac_address_primary: "aa:bb:cc:dd:ee:ff"
  mac_address_vf0: "aa:bb:cc:dd:ee:01"
  mac_address_vf1: "aa:bb:cc:dd:ee:02"
  
property_schema:
  - mac_address_primary
  - mac_address_vf0
  - mac_address_vf1
```

### Custom Hardware Properties

```yaml
properties:
  # Introspected
  pci_device_id: "8086:1521"
  driver: "igb"
  driver_version: "5.4.0"
  
  # User added
  asset_tag: "SRV-001-NIC-1"
  purchase_date: "2024-01-15"
  warranty_expiry: "2027-01-15"
  replacement_part_number: "X520-DA2"
  
property_schema:
  - pci_device_id
  - driver
  - driver_version
  - asset_tag
  - purchase_date
  - warranty_expiry
  - replacement_part_number
```

## Implementation

The `property_schema` is automatically maintained by:

1. **Introspection**: Populates initial fields
2. **User edits**: System detects new fields and adds to schema
3. **Validation**: Plugin enforces schema completeness
4. **Ledger**: Logs all property additions and value changes

