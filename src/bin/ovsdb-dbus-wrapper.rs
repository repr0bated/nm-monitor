//! D-Bus wrapper for OVSDB Unix socket
//! Exposes OVS operations via D-Bus by wrapping the Unix socket

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use zbus::{interface, ConnectionBuilder};

const OVSDB_SOCKET: &str = "/var/run/openvswitch/db.sock";

struct OvsdbWrapper;

#[interface(name = "org.openvswitch.ovsdb")]
impl OvsdbWrapper {
    /// Create OVS bridge
    async fn create_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        Self::transact(json!([
            "Open_vSwitch",
            {
                "op": "insert",
                "table": "Bridge",
                "row": { "name": bridge_name },
                "uuid-name": "new_bridge"
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [["bridges", "insert", ["named-uuid", "new_bridge"]]]
            }
        ]))
        .map(|_| ())
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Add port to bridge
    async fn add_port(&self, bridge_name: String, port_name: String) -> zbus::fdo::Result<()> {
        Self::transact(json!([
            "Open_vSwitch",
            {
                "op": "insert",
                "table": "Port",
                "row": { "name": port_name },
                "uuid-name": "new_port"
            },
            {
                "op": "insert",
                "table": "Interface",
                "row": { "name": port_name },
                "uuid-name": "new_iface"
            },
            {
                "op": "mutate",
                "table": "Port",
                "where": [["_uuid", "==", ["named-uuid", "new_port"]]],
                "mutations": [["interfaces", "insert", ["named-uuid", "new_iface"]]]
            },
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "mutations": [["ports", "insert", ["named-uuid", "new_port"]]]
            }
        ]))
        .map(|_| ())
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Delete bridge
    async fn delete_bridge(&self, bridge_name: String) -> zbus::fdo::Result<()> {
        Self::transact(json!([
            "Open_vSwitch",
            {
                "op": "delete",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]]
            }
        ]))
        .map(|_| ())
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Check if bridge exists
    async fn bridge_exists(&self, bridge_name: String) -> zbus::fdo::Result<bool> {
        let response = Self::transact(json!([
            "Open_vSwitch",
            {
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]]
            }
        ]))
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        if let Some(result) = response.get("result") {
            if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                return Ok(!rows.as_array().unwrap_or(&vec![]).is_empty());
            }
        }

        Ok(false)
    }

    /// List bridge ports
    async fn list_bridge_ports(&self, bridge_name: String) -> zbus::fdo::Result<Vec<String>> {
        let response = Self::transact(json!([
            "Open_vSwitch",
            {
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": ["ports"]
            }
        ]))
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let mut port_names = Vec::new();

        if let Some(result) = response.get("result") {
            if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                if let Some(rows_array) = rows.as_array() {
                    for row in rows_array {
                        if let Some(ports) = row.get("ports") {
                            if let Some(port_set) = ports.get(1).and_then(|p| p.as_array()) {
                                for uuid in port_set {
                                    if let Some(name) = Self::get_port_name(uuid)
                                        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
                                    {
                                        port_names.push(name);
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
}

impl OvsdbWrapper {
    /// Send transaction to OVSDB Unix socket
    fn transact(params: Value) -> Result<Value> {
        let request = json!({
            "method": "transact",
            "params": params,
            "id": 0
        });

        let request_str = serde_json::to_string(&request)?;
        
        let mut stream = UnixStream::connect(OVSDB_SOCKET)?;
        stream.write_all(request_str.as_bytes())?;
        stream.write_all(b"\n")?;

        let mut response_str = String::new();
        stream.read_to_string(&mut response_str)?;

        let response: Value = serde_json::from_str(&response_str)?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("OVSDB error: {}", error);
        }

        Ok(response)
    }

    /// Get port name from UUID
    fn get_port_name(uuid: &Value) -> Result<Option<String>> {
        let uuid_str = if let Some(arr) = uuid.as_array() {
            arr.get(1).and_then(|v| v.as_str())
        } else {
            None
        };

        if let Some(uuid_val) = uuid_str {
            let response = Self::transact(json!([
                "Open_vSwitch",
                {
                    "op": "select",
                    "table": "Port",
                    "where": [["_uuid", "==", ["uuid", uuid_val]]],
                    "columns": ["name"]
                }
            ]))?;

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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let _conn = ConnectionBuilder::system()?
        .name("org.openvswitch.ovsdb")?
        .serve_at("/org/openvswitch/ovsdb", OvsdbWrapper)?
        .build()
        .await?;

    println!("OVSDB D-Bus wrapper running at org.openvswitch.ovsdb");
    
    // Keep running
    std::future::pending::<()>().await;
    
    Ok(())
}
