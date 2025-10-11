use anyhow::{Context, Result};
use std::process::Command;

pub fn list_connection_names() -> Result<Vec<String>> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "c", "show"])
        .output()
        .with_context(|| "nmcli c show")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let names = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(names)
}

