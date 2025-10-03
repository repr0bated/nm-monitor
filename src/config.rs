use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub bridge_name: String,
    pub interfaces_path: String,
    pub include_prefixes: Vec<String>,
    pub debounce_ms: u64,
    pub managed_block_tag: String,
    pub naming_template: String,
    pub container_name_cmd: Option<String>,
    pub ledger_path: String,
    pub enable_rename: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bridge_name: "ovsbr0".to_string(),
            interfaces_path: "/etc/network/interfaces".to_string(),
            include_prefixes: vec!["veth".to_string(), "tap".to_string()],
            debounce_ms: 500,
            managed_block_tag: "ovs-port-agent".to_string(),
            naming_template: "veth-{container}-eth{index}".to_string(),
            container_name_cmd: None,
            ledger_path: "/var/lib/ovs-port-agent/ledger.jsonl".to_string(),
            enable_rename: false,
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        // Try explicit path, then /etc, then project-local config/example
        let candidates = if let Some(p) = path { vec![p.to_path_buf()] } else { vec![
            "/etc/ovs-port-agent/config.toml".into(),
            "config/config.toml".into(),
            "config/config.toml.example".into(),
        ]};

        for candidate in candidates {
            if candidate.exists() {
                let data = fs::read_to_string(&candidate)
                    .with_context(|| format!("reading config: {}", candidate.display()))?;
                let cfg: Config = toml::from_str(&data)
                    .with_context(|| format!("parsing config: {}", candidate.display()))?;
                return Ok(cfg);
            }
        }

        Ok(Config::default())
    }
}
