use anyhow::{Context, Result};
use std::{fs, path::Path};

pub fn update_interfaces_block(interfaces_path: &Path, tag: &str, port_names: &[String], bridge: &str) -> Result<()> {
    let begin_marker = format!("# BEGIN {tag}\n");
    let end_marker = format!("# END {tag}\n");

    let mut block = String::new();
    block.push_str(&begin_marker);
    block.push_str(&format!("# Managed by {tag}. Do not edit.\n"));

    if port_names.is_empty() {
        block.push_str("# No container OVS ports detected.\n");
    } else {
        for name in port_names {
            block.push_str(&format!(
                "allow-ovs {n}\niface {n} inet manual\n    ovs_type OVSPort\n    ovs_bridge {b}\n\n",
                n = name,
                b = bridge
            ));
        }
    }

    block.push_str(&end_marker);

    let content = fs::read_to_string(interfaces_path).unwrap_or_default();
    let new_content = replace_block(&content, &begin_marker, &end_marker, &block);

    if new_content != content {
        let path_display = interfaces_path.display().to_string();
        fs::write(interfaces_path, new_content)
            .with_context(|| format!("writing interfaces file: {path_display}"))?;
    }

    Ok(())
}

fn replace_block(content: &str, begin_marker: &str, end_marker: &str, new_block: &str) -> String {
    if let Some(start) = content.find(begin_marker) {
        if let Some(end) = content[start..].find(end_marker) {
            let end_idx = start + end + end_marker.len();
            let mut result = String::with_capacity(content.len() + new_block.len());
            result.push_str(&content[..start]);
            result.push_str(new_block);
            result.push_str(&content[end_idx..]);
            return result;
        }
    }

    let mut result = String::with_capacity(content.len() + new_block.len() + 1);
    result.push_str(content);
    if !content.ends_with('\n') { result.push('\n'); }
    result.push_str(new_block);
    result
}
