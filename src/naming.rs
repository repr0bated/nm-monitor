use sha1::{Digest, Sha1};

/// Build a Proxmox-safe interface name: containername_eth{index}
/// - Max 15 chars total
/// - Allowed: [A-Za-z0-9_]
/// - Replace other chars with '_'
/// - Deterministic, collision-safe (adds short hash if needed)
pub fn container_eth_name(container: &str, index: u16) -> String {
    let suffix = format!("_eth{}", index);
    let max_base_len = 15usize.saturating_sub(suffix.len());

    let mut base: String = container
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    if base.len() > max_base_len {
        base.truncate(max_base_len);
    }

    let candidate = format!("{base}{suffix}");
    if candidate.len() <= 15 {
        return candidate;
    }

    // Rare fallback: ensure uniqueness with 2-hex hash while respecting length
    let mut hasher = Sha1::new();
    hasher.update(container.as_bytes());
    let short = &hex::encode(hasher.finalize())[..2];

    let separator = "_";
    let reserved = suffix.len() + separator.len() + short.len();
    let mut trimmed = base;
    if trimmed.len() > 15 - reserved {
        trimmed.truncate(15 - reserved);
    }

    format!("{trimmed}{separator}{short}{suffix}")
}

/// Render from template like "vi{container}", sanitize, and trim to 15 chars
pub fn render_template(template: &str, container: &str, index: u16) -> String {
    let rendered = template
        .replace("{container}", container)
        .replace("{index}", &index.to_string());
    sanitize15(&rendered)
}

fn sanitize15(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if out.len() <= 15 {
        return out;
    }

    // Truncate to 15 chars with hash for uniqueness
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    let short = &hex::encode(hasher.finalize())[..2];
    let keep = 15usize.saturating_sub(2); // 2 hex chars at end
    let mut base: String = out.chars().take(keep).collect();
    base.push_str(short);
    base
}
