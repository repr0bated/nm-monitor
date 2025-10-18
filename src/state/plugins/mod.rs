// Plugin implementations
pub mod net;
pub mod netcfg;
pub mod docker;
pub mod netmaker;

pub use net::NetStatePlugin;
pub use netcfg::NetcfgStatePlugin;
pub use docker::DockerStatePlugin;
pub use netmaker::NetmakerStatePlugin;
