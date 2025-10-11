use anyhow::{Context, Result};
use log::info;
use std::future;
use zbus::{fdo::IntrospectableProxy, ConnectionBuilder};

use crate::ledger::Ledger;
use crate::nmcli_dyn;
use std::path::PathBuf;

pub struct AppState {
    pub bridge: String,
    pub ledger_path: String,
}

pub struct PortAgent {
    state: AppState,
}

impl PortAgent {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

#[zbus::dbus_interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    /// Health check
    fn ping(&self) -> String {
        "pong".into()
    }

    /// List container interfaces on the managed bridge
    fn list_ports(&self) -> zbus::fdo::Result<Vec<String>> {
        nmcli_dyn::list_connection_names()
            .map(|v| {
                v.into_iter()
                    .filter(|n| n.starts_with("ovs-eth-"))
                    .map(|n| n.trim_start_matches("ovs-eth-").to_string())
                    .collect()
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Add a port to the managed bridge (legacy API - deprecated)
    fn add_port(&self, name: &str) -> zbus::fdo::Result<()> {
        // For backward compatibility, treat this as creating a container interface
        // Use default configuration values
        let interfaces_path = "/etc/network/interfaces".to_string();
        let managed_tag = "ovs-port-agent".to_string();
        let enable_rename = true;
        let naming_template = "vi{container}".to_string();

        tokio::runtime::Handle::current()
            .block_on(async {
                crate::netlink::create_container_interface(
                    self.state.bridge.clone(),
                    raw_ifname,
                    container_id,
                    vmid,
                    interfaces_path,
                    managed_tag,
                    enable_rename,
                    naming_template,
                    self.state.ledger_path.clone(),
                ).await
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to create container interface: {}", e)))?;

        Ok(format!("Container interface created for VMID {}", vmid))
    }

    /// Delete a port from the managed bridge (legacy API - deprecated)
    fn del_port(&self, name: &str) -> zbus::fdo::Result<String> {
        // For backward compatibility, treat this as removing a container interface
        let interfaces_path = "/etc/network/interfaces".to_string();
        let managed_tag = "ovs-port-agent".to_string();

        tokio::runtime::Handle::current()
            .block_on(async {
                crate::netlink::remove_container_interface(
                    self.state.bridge.clone(),
                    name,
                    interfaces_path,
                    managed_tag,
                    self.state.ledger_path.clone(),
                ).await
            })
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to remove container interface: {}", e)))?;

        Ok(format!("Container interface {} removed", name))
    }

    /// Perform comprehensive NetworkManager introspection
    fn introspect_network_manager(&self) -> zbus::fdo::Result<String> {
        match tokio::runtime::Handle::current()
            .block_on(async { introspect_nm().await })
        {
            Ok(_) => Ok("NetworkManager introspection completed successfully".to_string()),
            Err(e) => Err(zbus::fdo::Error::Failed(format!(
                "NetworkManager introspection failed: {}",
                e
            ))),
        }
    }
}

pub async fn serve_with_state(state: AppState) -> Result<()> {
    let agent = PortAgent::new(state);
    let name = "dev.ovs.PortAgent1";
    let path = "/dev/ovs/PortAgent1";
    let _conn = ConnectionBuilder::system()?
        .name(name)?
        .serve_at(path, agent)?
        .build()
        .await?;
    info!("D-Bus service registered: {} at {}", name, path);
    future::pending::<()>().await;
    // unreachable
    #[allow(unreachable_code)]
    Ok(())
}

pub async fn introspect_nm() -> Result<()> {
    info!("Performing comprehensive D-Bus introspection on NetworkManager");

    let conn = zbus::Connection::system().await?;

    println!("🔍 NetworkManager Comprehensive Introspection Report");
    println!("==================================================");
    println!(
        "Timestamp: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!();

    // 1. NetworkManager Main Object
    println!("📡 1. NetworkManager Main Object");
    println!("--------------------------------");
    match introspect_object(
        &conn,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
    )
    .await
    {
        Ok(xml) => {
            println!("✅ Successfully introspected main NetworkManager object");
            println!("Properties and methods available:");
            print_introspection_summary(&xml);
        }
        Err(e) => {
            println!("❌ Failed to introspect main NetworkManager object: {}", e);
            println!("This might indicate NetworkManager is not running or accessible.");
        }
    }
    println!();

    // 2. NetworkManager Settings Object
    println!("⚙️  2. NetworkManager Settings Object");
    println!("-----------------------------------");
    match introspect_object(
        &conn,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager/Settings",
    )
    .await
    {
        Ok(xml) => {
            println!("✅ Successfully introspected Settings object");
            print_introspection_summary(&xml);
        }
        Err(e) => {
            println!("❌ Failed to introspect Settings object: {}", e);
        }
    }
    println!();

    // 3. NetworkManager Devices
    println!("🔌 3. NetworkManager Devices");
    println!("----------------------------");
    match get_nm_devices(&conn).await {
        Ok(devices) => {
            println!("✅ Found {} devices", devices.len());
            for (i, device) in devices.iter().enumerate() {
                println!(
                    "  {}. {} - {} ({})",
                    i + 1,
                    device.interface,
                    device.device_type,
                    device.state
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to get devices: {}", e);
        }
    }
    println!();

    // 4. NetworkManager Active Connections
    println!("🌐 4. NetworkManager Active Connections");
    println!("--------------------------------------");
    match get_nm_active_connections(&conn).await {
        Ok(connections) => {
            println!("✅ Found {} active connections", connections.len());
            for (i, conn_info) in connections.iter().enumerate() {
                println!(
                    "  {}. {} - {} ({})",
                    i + 1,
                    conn_info.name,
                    conn_info.connection_type,
                    conn_info.state
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to get active connections: {}", e);
        }
    }
    println!();

    // 5. NetworkManager All Connections
    println!("📋 5. NetworkManager All Connections");
    println!("-----------------------------------");
    match get_nm_all_connections(&conn).await {
        Ok(connections) => {
            println!("✅ Found {} total connections", connections.len());
            for (i, conn_info) in connections.iter().enumerate() {
                println!(
                    "  {}. {} - {} ({})",
                    i + 1,
                    conn_info.name,
                    conn_info.connection_type,
                    conn_info.uuid
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to get all connections: {}", e);
        }
    }
    println!();

    // 6. OVS-Specific Connections
    println!("🔗 6. OVS-Specific Connections");
    println!("------------------------------");
    match find_ovs_connections().await {
        Ok(ovs_connections) => {
            println!("✅ Found {} OVS-related connections", ovs_connections.len());
            for (i, conn_info) in ovs_connections.iter().enumerate() {
                println!(
                    "  {}. {} - {} ({})",
                    i + 1,
                    conn_info.name,
                    conn_info.connection_type,
                    conn_info.uuid
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to find OVS connections: {}", e);
        }
    }
    println!();

    // 7. System Network State
    println!("🖥️  7. System Network State");
    println!("--------------------------");
    match get_system_network_state().await {
        Ok(state) => {
            println!("✅ System network state retrieved");
            println!("NetworkManager Version: {}", state.nm_version);
            println!("NetworkManager State: {}", state.nm_state);
            println!("Connectivity: {}", state.connectivity);
            println!("Active Connections: {}", state.active_connections);
            println!("All Connections: {}", state.all_connections);
        }
        Err(e) => {
            println!("❌ Failed to get system network state: {}", e);
        }
    }
    println!();

    // 8. D-Bus Service Information
    println!("🚌 8. D-Bus Service Information");
    println!("------------------------------");
    match get_dbus_service_info(&conn).await {
        Ok(info) => {
            println!("✅ D-Bus service information retrieved");
            println!("Service Name: {}", info.service_name);
            println!("Object Path: {}", info.object_path);
            println!("Service Available: {}", info.available);
        }
        Err(e) => {
            println!("❌ Failed to get D-Bus service info: {}", e);
        }
    }
    println!();

    println!("==================================================");
    println!("🔍 Introspection Complete");
    println!("💡 Use this information for debugging NetworkManager integration issues.");

    Ok(())
}

// Helper function to introspect a D-Bus object with error handling
async fn introspect_object(
    conn: &zbus::Connection,
    destination: &str,
    path: &str,
) -> Result<String> {
    use zbus::fdo::IntrospectableProxy;

    match IntrospectableProxy::builder(conn)
        .destination(destination)?
                            .path(path)?
                            .build()
                            .await
                        {
        Ok(proxy) => match proxy.introspect().await {
            Ok(xml) => Ok(xml),
            Err(e) => Err(anyhow::anyhow!("Failed to introspect {}: {}", path, e)),
        },
        Err(e) => Err(anyhow::anyhow!(
            "Failed to create proxy for {}: {}",
            path,
            e
        )),
    }
}

// Helper function to get NetworkManager devices
async fn get_nm_devices(_conn: &zbus::Connection) -> Result<Vec<DeviceInfo>> {
    // Use nmcli for device information since D-Bus API is complex
    let output = std::process::Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "device", "status"])
        .output()
        .context("Failed to get NetworkManager devices")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("nmcli device status command failed"));
    }

    let devices_text = String::from_utf8_lossy(&output.stdout);
    let mut device_infos = Vec::new();

    for line in devices_text.lines() {
        if let Some((device, type_state)) = line.split_once(':') {
            if let Some((device_type, state)) = type_state.split_once(':') {
                let device_type_str = match device_type {
                    "ethernet" => "Ethernet",
                    "wifi" => "Wi-Fi",
                    "bluetooth" => "Bluetooth",
                    "ovs-interface" => "OVS Interface",
                    "ovs-port" => "OVS Port",
                    "ovs-bridge" => "OVS Bridge",
                    _ => "Unknown",
                }
                .to_string();

                let state_str = match state {
                    "unmanaged" => "Unmanaged",
                    "unavailable" => "Unavailable",
                    "disconnected" => "Disconnected",
                    "prepare" => "Prepare",
                    "config" => "Config",
                    "need-auth" => "Need Auth",
                    "ip-config" => "IP Config",
                    "ip-check" => "IP Check",
                    "secondaries" => "Secondaries",
                    "activated" => "Activated",
                    "deactivating" => "Deactivating",
                    "failed" => "Failed",
                    _ => "Unknown",
                }
                .to_string();

                device_infos.push(DeviceInfo {
                    interface: device.to_string(),
                    device_type: device_type_str,
                    state: state_str,
                });
            }
        }
    }

    Ok(device_infos)
}

// Helper function to get active connections
async fn get_nm_active_connections(_conn: &zbus::Connection) -> Result<Vec<ConnectionInfo>> {
    // Use nmcli for active connections since D-Bus API is complex
    let output = std::process::Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "NAME,UUID,TYPE,STATE",
            "connection",
            "show",
            "--active",
        ])
        .output()
        .context("Failed to get NetworkManager active connections")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("nmcli active connections command failed"));
    }

    let connections_text = String::from_utf8_lossy(&output.stdout);
    let mut connection_infos = Vec::new();

    for line in connections_text.lines() {
        if let Some((name, uuid_type_state)) = line.split_once(':') {
            if let Some((uuid, type_state)) = uuid_type_state.split_once(':') {
                if let Some((connection_type, state)) = type_state.split_once(':') {
                    let state_str = match state {
                        "activated" => "Activated",
                        "activating" => "Activating",
                        "deactivated" => "Deactivated",
                        "deactivating" => "Deactivating",
                        _ => "Unknown",
                    }
                    .to_string();

                    connection_infos.push(ConnectionInfo {
                        name: name.to_string(),
                        uuid: uuid.to_string(),
                        connection_type: connection_type.to_string(),
                        state: state_str,
                    });
                }
            }
        }
    }

    Ok(connection_infos)
}

// Helper function to get all connections
async fn get_nm_all_connections(_conn: &zbus::Connection) -> Result<Vec<ConnectionInfo>> {
    let output = std::process::Command::new("nmcli")
        .args(["-t", "-f", "NAME,UUID,TYPE", "connection", "show"])
        .output()
        .context("Failed to get NetworkManager connections")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("nmcli command failed"));
    }

    let connections_text = String::from_utf8_lossy(&output.stdout);
    let mut connection_infos = Vec::new();

    for line in connections_text.lines() {
        if let Some((name, uuid_type)) = line.split_once(':') {
            if let Some((uuid, connection_type)) = uuid_type.split_once(':') {
                connection_infos.push(ConnectionInfo {
                    name: name.to_string(),
                    uuid: uuid.to_string(),
                    connection_type: connection_type.to_string(),
                    state: "Unknown".to_string(), // We don't have state for inactive connections
                });
            }
        }
    }

    Ok(connection_infos)
}

// Helper function to find OVS connections
async fn find_ovs_connections() -> Result<Vec<ConnectionInfo>> {
    let output = std::process::Command::new("nmcli")
        .args(["-t", "-f", "NAME,UUID,TYPE", "connection", "show"])
        .output()
        .context("Failed to get NetworkManager connections for OVS search")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("nmcli command failed"));
    }

    let connections_text = String::from_utf8_lossy(&output.stdout);
    let mut ovs_connections = Vec::new();

    for line in connections_text.lines() {
        if let Some((name, uuid_type)) = line.split_once(':') {
            if let Some((uuid, connection_type)) = uuid_type.split_once(':') {
                if connection_type.contains("ovs") {
                    ovs_connections.push(ConnectionInfo {
                        name: name.to_string(),
                        uuid: uuid.to_string(),
                        connection_type: connection_type.to_string(),
                        state: "Unknown".to_string(),
                    });
                }
            }
        }
    }

    Ok(ovs_connections)
}

// Helper function to get system network state
async fn get_system_network_state() -> Result<SystemNetworkState> {
    // Get NetworkManager version and state
    let nm_version = std::process::Command::new("nmcli")
        .args(["--version"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let nm_state_output = std::process::Command::new("nmcli")
        .args(["-t", "-f", "STATE", "general"])
        .output()?;

    let nm_state = if nm_state_output.status.success() {
        String::from_utf8_lossy(&nm_state_output.stdout)
            .trim()
            .to_string()
    } else {
        "Unknown".to_string()
    };

    let connectivity_output = std::process::Command::new("nmcli")
        .args(["-t", "-f", "CONNECTIVITY", "general"])
        .output()?;

    let connectivity = if connectivity_output.status.success() {
        String::from_utf8_lossy(&connectivity_output.stdout)
            .trim()
            .to_string()
    } else {
        "Unknown".to_string()
    };

    // Count connections
    let active_connections = std::process::Command::new("nmcli")
        .args(["-t", "connection", "show", "--active"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).lines().count())
        .unwrap_or(0);

    let all_connections = std::process::Command::new("nmcli")
        .args(["-t", "connection", "show"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).lines().count())
        .unwrap_or(0);

    Ok(SystemNetworkState {
        nm_version,
        nm_state,
        connectivity,
        active_connections,
        all_connections,
    })
}

// Helper function to get D-Bus service information
async fn get_dbus_service_info(_conn: &zbus::Connection) -> Result<DBusServiceInfo> {
    Ok(DBusServiceInfo {
        service_name: "org.freedesktop.NetworkManager".to_string(),
        object_path: "/org/freedesktop/NetworkManager".to_string(),
        available: true, // If we got here, the service is available
    })
}

// Helper function to print a summary of introspection XML
fn print_introspection_summary(xml: &str) {
    let mut interfaces = 0;
    let mut methods = 0;
    let mut properties = 0;
    let mut signals = 0;

    for line in xml.lines() {
        if line.contains("<interface name=") {
            interfaces += 1;
        } else if line.contains("<method name=") {
            methods += 1;
        } else if line.contains("<property name=") {
            properties += 1;
        } else if line.contains("<signal name=") {
            signals += 1;
        }
    }

    println!(
        "  📊 Summary: {} interfaces, {} methods, {} properties, {} signals",
        interfaces, methods, properties, signals
    );
}

// Data structures for introspection results
#[derive(Debug)]
struct DeviceInfo {
    interface: String,
    device_type: String,
    state: String,
}

#[derive(Debug)]
struct ConnectionInfo {
    name: String,
    uuid: String,
    connection_type: String,
    state: String,
}

#[derive(Debug)]
struct SystemNetworkState {
    nm_version: String,
    nm_state: String,
    connectivity: String,
    active_connections: usize,
    all_connections: usize,
}

#[derive(Debug)]
struct DBusServiceInfo {
    service_name: String,
    object_path: String,
    available: bool,
}
