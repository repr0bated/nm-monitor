//! OVSDB client using Unix socket JSON-RPC
//! OVS doesn't have a D-Bus interface, uses Unix socket instead

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

const OVSDB_SOCKET: &str = "/var/run/openvswitch/db.sock";

/// OVSDB client for OVS operations via Unix socket
pub struct OvsdbClient {
    socket_path: String,
}

impl OvsdbClient {
    /// Connect to OVSDB Unix socket
    pub async fn new() -> Result<Self> {
        Ok(Self {
            socket_path: OVSDB_SOCKET.to_string(),
        })
    }

    /// Send JSON-RPC request to OVSDB
    fn transact(&self, params: Value) -> Result<Value> {
        let request = json!({
            "method": "transact",
            "params": params,
            "id": 0
        });

        let request_str = serde_json::to_string(&request)?;
        
        let mut stream = UnixStream::connect(&self.socket_path)
            .context("Failed to connect to OVSDB socket")?;

        stream.write_all(request_str.as_bytes())?;
        stream.write_all(b"\n")?;

        let mut response_str = String::new();
        stream.read_to_string(&mut response_str)?;

        let response: Value = serde_json::from_str(&response_str)
            .context("Failed to parse OVSDB response")?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("OVSDB error: {}", error);
        }

        Ok(response)
    }

    /// Create OVS bridge
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        let params = json!([
            "Open_vSwitch",
            {
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": bridge_name
                },
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

        self.transact(params)?;
        Ok(())
    }

    /// Add port to bridge
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        let params = json!([
            "Open_vSwitch",
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": port_name
                },
                "uuid-name": "new_port"
            },
            {
                "op": "insert",
                "table": "Interface",
                "row": {
                    "name": port_name
                },
                "uuid-name": "new_iface"
            },
            {
                "op": "mutate",
                "table": "Port",
                "where": [["_uuid", "==", ["named-uuid", "new_port"]]],
                "mutations": [
                    ["interfaces", "insert", ["named-uuid", "new_iface"]]
                ]
            },
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "mutations": [
                    ["ports", "insert", ["named-uuid", "new_port"]]
                ]
            }
        ]);

        self.transact(params)?;
        Ok(())
    }

    /// Delete bridge
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        let params = json!([
            "Open_vSwitch",
            {
                "op": "delete",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]]
            }
        ]);

        self.transact(params)?;
        Ok(())
    }

    /// Check if bridge exists
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        let params = json!([
            "Open_vSwitch",
            {
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]]
            }
        ]);

        let response = self.transact(params)?;
        
        if let Some(result) = response.get("result") {
            if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                return Ok(!rows.as_array().unwrap_or(&vec![]).is_empty());
            }
        }

        Ok(false)
    }

    /// List all ports on a bridge
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        let params = json!([
            "Open_vSwitch",
            {
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": ["ports"]
            }
        ]);

        let response = self.transact(params)?;
        let mut port_names = Vec::new();

        if let Some(result) = response.get("result") {
            if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                if let Some(rows_array) = rows.as_array() {
                    for row in rows_array {
                        if let Some(ports) = row.get("ports") {
                            if let Some(port_set) = ports.get(1).and_then(|p| p.as_array()) {
                                for uuid in port_set {
                                    if let Some(port_name) = self.get_port_name(uuid)? {
                                        port_names.push(port_name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(port_names)
    }

    /// Get port name from UUID
    fn get_port_name(&self, uuid: &Value) -> Result<Option<String>> {
        let uuid_str = if let Some(arr) = uuid.as_array() {
            arr.get(1).and_then(|v| v.as_str())
        } else {
            None
        };

        if let Some(uuid_val) = uuid_str {
            let params = json!([
                "Open_vSwitch",
                {
                    "op": "select",
                    "table": "Port",
                    "where": [["_uuid", "==", ["uuid", uuid_val]]],
                    "columns": ["name"]
                }
            ]);

            let response = self.transact(params)?;
            
            if let Some(result) = response.get("result") {
                if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                    if let Some(first_row) = rows.as_array().and_then(|a| a.first()) {
                        if let Some(name) = first_row.get("name").and_then(|n| n.as_str()) {
                            return Ok(Some(name.to_string()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}
