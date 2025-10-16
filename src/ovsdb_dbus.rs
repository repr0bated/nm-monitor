//! OVSDB D-Bus interface for OVS bridge operations
//! Replaces ovs-vsctl with proper D-Bus calls

use anyhow::{Context, Result};
use serde_json::{json, Value};
use zbus::{Connection, Proxy};

/// OVSDB D-Bus client for OVS operations
pub struct OvsdbClient {
    proxy: Proxy<'static>,
}

impl OvsdbClient {
    /// Connect to OVSDB D-Bus service
    pub async fn new() -> Result<Self> {
        let conn = Connection::system().await
            .context("Failed to connect to system D-Bus")?;
        
        let proxy = Proxy::new(
            &conn,
            "org.openvswitch.ovsdb",
            "/org/openvswitch/ovsdb",
            "org.openvswitch.ovsdb",
        ).await.context("Failed to create OVSDB proxy")?;

        Ok(Self { proxy })
    }

    /// Create OVS bridge via D-Bus
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        let request = json!({
            "method": "transact",
            "params": [
                "Open_vSwitch",
                {
                    "op": "insert",
                    "table": "Bridge",
                    "row": {
                        "name": bridge_name
                    },
                    "uuid-name": "bridge"
                },
                {
                    "op": "mutate",
                    "table": "Open_vSwitch",
                    "where": [],
                    "mutations": [
                        ["bridges", "insert", ["named-uuid", "bridge"]]
                    ]
                }
            ]
        });

        let request_str = serde_json::to_string(&request)?;
        let response_str: String = self.proxy.call("transact", &(request_str,)).await
            .context("Failed to call OVSDB transact")?;

        let response: Value = serde_json::from_str(&response_str)?;
        if response.get("error").is_some() {
            anyhow::bail!("OVSDB error creating bridge: {}", response);
        }

        Ok(())
    }

    /// Add port to bridge via D-Bus
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        let request = json!({
            "method": "transact",
            "params": [
                "Open_vSwitch",
                {
                    "op": "insert",
                    "table": "Port",
                    "row": {
                        "name": port_name
                    },
                    "uuid-name": "port"
                },
                {
                    "op": "insert",
                    "table": "Interface",
                    "row": {
                        "name": port_name
                    },
                    "uuid-name": "interface"
                },
                {
                    "op": "mutate",
                    "table": "Port",
                    "where": [["_uuid", "==", ["named-uuid", "port"]]],
                    "mutations": [
                        ["interfaces", "insert", ["named-uuid", "interface"]]
                    ]
                },
                {
                    "op": "mutate",
                    "table": "Bridge",
                    "where": [["name", "==", bridge_name]],
                    "mutations": [
                        ["ports", "insert", ["named-uuid", "port"]]
                    ]
                }
            ]
        });

        let request_str = serde_json::to_string(&request)?;
        let response_str: String = self.proxy.call("transact", &(request_str,)).await
            .context("Failed to call OVSDB transact")?;

        let response: Value = serde_json::from_str(&response_str)?;
        if response.get("error").is_some() {
            anyhow::bail!("OVSDB error adding port: {}", response);
        }

        Ok(())
    }

    /// Delete bridge via D-Bus
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        let request = json!({
            "method": "transact",
            "params": [
                "Open_vSwitch",
                {
                    "op": "delete",
                    "table": "Bridge",
                    "where": [["name", "==", bridge_name]]
                }
            ]
        });

        let request_str = serde_json::to_string(&request)?;
        let response_str: String = self.proxy.call("transact", &(request_str,)).await
            .context("Failed to call OVSDB transact")?;

        let response: Value = serde_json::from_str(&response_str)?;
        if response.get("error").is_some() {
            anyhow::bail!("OVSDB error deleting bridge: {}", response);
        }

        Ok(())
    }

    /// Check if bridge exists via D-Bus
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        let request = json!({
            "method": "transact",
            "params": [
                "Open_vSwitch",
                {
                    "op": "select",
                    "table": "Bridge",
                    "where": [["name", "==", bridge_name]]
                }
            ]
        });

        let request_str = serde_json::to_string(&request)?;
        let response_str: String = self.proxy.call("transact", &(request_str,)).await
            .context("Failed to call OVSDB transact")?;

        let response: Value = serde_json::from_str(&response_str)?;
        if let Some(result) = response.get("result") {
            if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                return Ok(!rows.as_array().unwrap_or(&vec![]).is_empty());
            }
        }

        Ok(false)
    }

    /// List all ports on a bridge via D-Bus
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        let request = json!({
            "method": "transact",
            "params": [
                "Open_vSwitch",
                {
                    "op": "select",
                    "table": "Bridge",
                    "where": [["name", "==", bridge_name]],
                    "columns": ["ports"]
                }
            ]
        });

        let request_str = serde_json::to_string(&request)?;
        let response_str: String = self.proxy.call("transact", &(request_str,)).await
            .context("Failed to call OVSDB transact")?;

        let response: Value = serde_json::from_str(&response_str)?;
        let mut port_names = Vec::new();

        if let Some(result) = response.get("result") {
            if let Some(rows) = result.get(0).and_then(|r| r.get("rows")) {
                if let Some(rows_array) = rows.as_array() {
                    for row in rows_array {
                        if let Some(ports) = row.get("ports") {
                            if let Some(port_uuids) = ports.as_array() {
                                for uuid in port_uuids {
                                    if let Some(port_name) = self.get_port_name(uuid).await? {
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
    async fn get_port_name(&self, uuid: &Value) -> Result<Option<String>> {
        let uuid_str = if let Some(arr) = uuid.as_array() {
            arr.get(1).and_then(|v| v.as_str())
        } else {
            None
        };

        if let Some(uuid_val) = uuid_str {
            let request = json!({
                "method": "transact",
                "params": [
                    "Open_vSwitch",
                    {
                        "op": "select",
                        "table": "Port",
                        "where": [["_uuid", "==", ["uuid", uuid_val]]],
                        "columns": ["name"]
                    }
                ]
            });

            let request_str = serde_json::to_string(&request)?;
            let response_str: String = self.proxy.call("transact", &(request_str,)).await
                .context("Failed to call OVSDB transact")?;

            let response: Value = serde_json::from_str(&response_str)?;
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
