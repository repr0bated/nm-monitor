use anyhow::Result;
use zbus::ConnectionBuilder;
use log::info;

struct PortAgent;

#[zbus::dbus_interface(name = "dev.ovs.PortAgent1")]
impl PortAgent {
    /// Example ping method
    fn ping(&self) -> String { "pong".into() }
}

pub async fn serve() -> Result<()> {
    let agent = PortAgent;
    let name = "dev.ovs.PortAgent1";
    let path = "/dev/ovs/PortAgent1";
    let _conn = ConnectionBuilder::system()?
        .name(name)?
        .serve_at(path, agent)?
        .build()
        .await?;
    info!("D-Bus service registered: {} at {}", name, path);
    Ok(())
}

pub async fn introspect_nm() -> Result<()> {
    use zbus::fdo::IntrospectableProxy;
    let conn = zbus::Connection::system().await?;
    let proxy = IntrospectableProxy::builder(&conn)
        .destination("org.freedesktop.NetworkManager")?
        .path("/org/freedesktop/NetworkManager")?
        .build()
        .await?;
    let xml = proxy.introspect().await?;
    println!("{}", xml);
    Ok(())
}
