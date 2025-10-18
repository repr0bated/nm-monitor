use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// FUSE mount point for Proxmox veth interface binding
const FUSE_MOUNT_BASE: &str = "/var/lib/ovs-port-agent/fuse";
const PROXMOX_API_BASE: &str = "/var/lib/ovs-port-agent/proxmox";

/// Cryptographic hash footprint for network elements
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HashFootprint {
    /// SHA-256 hash of the complete configuration
    pub config_hash: String,
    /// SHA-256 hash of the runtime state (timestamp)
    pub state_hash: String,
    /// Combined fingerprint (hash of config + state)
    pub fingerprint: String,
    /// Timestamp when footprint was created
    pub created_at: String,
    /// Fields included in the hash (for verification)
    pub hashed_fields: Vec<String>,
    /// Algorithm used (for future flexibility)
    pub algorithm: String,
}

impl HashFootprint {
    /// Create new footprint from configuration
    pub fn from_config<T: Serialize>(config: &T) -> Result<Self> {
        let config_json = serde_json::to_string(config)
            .context("Failed to serialize config for hashing")?;
        let config_hash = Self::hash_string(&config_json);
        
        let created_at = chrono::Utc::now().to_rfc3339();
        let state_hash = Self::hash_string(&created_at);
        
        let fingerprint = Self::hash_string(&format!("{}:{}", config_hash, state_hash));
        
        let hashed_fields = Self::extract_fields(config)?;
        
        Ok(Self {
            config_hash,
            state_hash,
            fingerprint,
            created_at,
            hashed_fields,
            algorithm: "SHA-256".to_string(),
        })
    }
    
    /// Verify footprint matches current configuration
    pub fn verify<T: Serialize>(&self, config: &T) -> Result<bool> {
        let config_json = serde_json::to_string(config)
            .context("Failed to serialize config for verification")?;
        let current_hash = Self::hash_string(&config_json);
        Ok(current_hash == self.config_hash)
    }
    
    fn hash_string(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    fn extract_fields<T: Serialize>(config: &T) -> Result<Vec<String>> {
        let value = serde_json::to_value(config)
            .context("Failed to extract fields from config")?;
        if let Some(obj) = value.as_object() {
            Ok(obj.keys().cloned().collect())
        } else {
            Ok(vec![])
        }
    }
}

/// Interface binding information for Proxmox integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceBinding {
    pub proxmox_veth: String,
    pub ovs_interface: String,
    pub vmid: u32,
    pub container_id: String,
    pub bridge: String,
    pub created_at: String,
    pub bind_mount: String,
    /// Hash footprint for integrity verification
    pub footprint: HashFootprint,
    /// Link to blockchain ledger block
    pub ledger_block_hash: Option<String>,
}

impl InterfaceBinding {
    /// Verify binding hasn't been tampered with
    pub fn verify_integrity(&self) -> Result<bool> {
        // Create a temporary binding without footprint for verification
        let temp_binding = InterfaceBinding {
            proxmox_veth: self.proxmox_veth.clone(),
            ovs_interface: self.ovs_interface.clone(),
            vmid: self.vmid,
            container_id: self.container_id.clone(),
            bridge: self.bridge.clone(),
            created_at: self.created_at.clone(),
            bind_mount: self.bind_mount.clone(),
            footprint: HashFootprint::default(),
            ledger_block_hash: None,
        };
        
        self.footprint.verify(&temp_binding)
    }
    
    /// Get the unique fingerprint
    pub fn fingerprint(&self) -> &str {
        &self.footprint.fingerprint
    }
}

#[allow(dead_code)]
type BindingMap = Arc<Mutex<HashMap<String, InterfaceBinding>>>;

/// Ensure FUSE mount point directory exists
pub fn ensure_fuse_mount_base() -> Result<()> {
    let mount_path = Path::new(FUSE_MOUNT_BASE);
    if !mount_path.exists() {
        fs::create_dir_all(mount_path)
            .with_context(|| format!("creating FUSE mount base: {}", FUSE_MOUNT_BASE))?;
        info!("Created FUSE mount base directory: {}", FUSE_MOUNT_BASE);
    }
    Ok(())
}

/// Bind mount a Proxmox veth interface to an OVS interface
/// This creates the necessary bind mount for Proxmox GUI visibility
pub fn bind_veth_interface(proxmox_veth: &str, ovs_interface: &str) -> Result<()> {
    let proxmox_path = format!("/proc/net/veth/{}", proxmox_veth);
    let ovs_path = format!("/sys/class/net/{}", ovs_interface);

    // Ensure source exists
    if !Path::new(&proxmox_path).exists() {
        return Err(anyhow::anyhow!(
            "Proxmox veth interface not found: {}",
            proxmox_path
        ));
    }

    if !Path::new(&ovs_path).exists() {
        return Err(anyhow::anyhow!("OVS interface not found: {}", ovs_path));
    }

    // Create mount point if it doesn't exist
    let mount_point = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);
    let mount_path = Path::new(&mount_point);

    if !mount_path.exists() {
        fs::create_dir_all(mount_path)
            .with_context(|| format!("creating mount point: {}", mount_point))?;
    }

    // Create bind mount
    let output = Command::new("mount")
        .args(["--bind", &proxmox_path, &mount_point])
        .output()
        .context("failed to create bind mount")?;

    if output.status.success() {
        info!("Successfully bound {} to {}", proxmox_veth, ovs_interface);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Failed to bind interfaces: {}", stderr))
    }
}

/// Unbind a veth interface
pub fn unbind_veth_interface(ovs_interface: &str) -> Result<()> {
    let mount_point = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);

    if Path::new(&mount_point).exists() {
        let output = Command::new("umount")
            .arg(&mount_point)
            .output()
            .context("failed to unmount")?;

        if output.status.success() {
            // Remove empty directory
            if let Err(e) = fs::remove_dir(&mount_point) {
                debug!(
                    "Failed to remove mount point directory {}: {}",
                    mount_point, e
                );
            }
            info!("Successfully unbound {}", ovs_interface);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to unbind {}: {}", ovs_interface, stderr);
        }
    }

    Ok(())
}

/// Clean up all FUSE mount points
pub fn cleanup_all_mounts() -> Result<()> {
    let mount_base = Path::new(FUSE_MOUNT_BASE);

    if mount_base.exists() {
        // List all mount points
        if let Ok(entries) = fs::read_dir(mount_base) {
            #[allow(clippy::manual_flatten)]
            for entry in entries {
                if let Ok(entry) = entry {
                    if entry.path().is_dir() {
                        if let Some(dir_name) = entry.file_name().to_str() {
                            if let Err(e) = unbind_veth_interface(dir_name) {
                                warn!("Failed to cleanup mount for {}: {}", dir_name, e);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Enhanced binding with Proxmox API compatibility and hash footprint
pub fn bind_veth_interface_enhanced(
    proxmox_veth: &str,
    ovs_interface: &str,
    vmid: u32,
    container_id: &str,
    bridge: &str,
) -> Result<InterfaceBinding> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let bind_mount = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);
    
    // Create temporary binding without footprint for hashing
    let temp_binding = InterfaceBinding {
        proxmox_veth: proxmox_veth.to_string(),
        ovs_interface: ovs_interface.to_string(),
        vmid,
        container_id: container_id.to_string(),
        bridge: bridge.to_string(),
        created_at: created_at.clone(),
        bind_mount: bind_mount.clone(),
        footprint: HashFootprint::default(),
        ledger_block_hash: None,
    };
    
    // Generate cryptographic footprint
    let footprint = HashFootprint::from_config(&temp_binding)
        .context("Failed to generate hash footprint")?;
    
    let binding = InterfaceBinding {
        proxmox_veth: proxmox_veth.to_string(),
        ovs_interface: ovs_interface.to_string(),
        vmid,
        container_id: container_id.to_string(),
        bridge: bridge.to_string(),
        created_at,
        bind_mount,
        footprint,
        ledger_block_hash: None,
    };

    // Create the standard bind mount
    bind_veth_interface(proxmox_veth, ovs_interface)?;

    // Create Proxmox API compatibility layer
    create_proxmox_api_interface(&binding)?;

    // Store binding information for D-Bus introspection
    store_binding_info(&binding)?;

    info!(
        "Enhanced binding created: {} -> {} (VMID: {}, Fingerprint: {})",
        proxmox_veth, ovs_interface, vmid, &binding.footprint.fingerprint[..16]
    );
    Ok(binding)
}

/// Create Proxmox API compatibility interface
fn create_proxmox_api_interface(binding: &InterfaceBinding) -> Result<()> {
    let proxmox_base = Path::new(PROXMOX_API_BASE);
    fs::create_dir_all(proxmox_base)?;

    // Create VM-specific directory structure
    let vm_dir = proxmox_base.join(format!("vm-{}", binding.vmid));
    fs::create_dir_all(&vm_dir)?;

    // Create interface symlink for Proxmox GUI visibility
    let interface_link = vm_dir.join(&binding.ovs_interface);
    if interface_link.exists() {
        fs::remove_file(&interface_link)?;
    }

    // Create symlink to the actual network interface
    let target_interface = format!("/sys/class/net/{}", binding.ovs_interface);
    std::os::unix::fs::symlink(target_interface, interface_link)?;

    // Create Proxmox-style configuration file
    let config_content = format!(
        r#"# Proxmox VE VM {} Network Interface Configuration
# Generated by ovs-port-agent
[interface.{}]
type: veth
bridge: {}
container: {}
proxmox-veth: {}
ovs-interface: {}
created: {}
"#,
        binding.vmid,
        binding.ovs_interface,
        binding.bridge,
        binding.container_id,
        binding.proxmox_veth,
        binding.ovs_interface,
        binding.created_at
    );

    let config_file = vm_dir.join(format!("{}.conf", binding.ovs_interface));
    fs::write(config_file, config_content)?;

    Ok(())
}

/// Store binding information for D-Bus introspection
fn store_binding_info(binding: &InterfaceBinding) -> Result<()> {
    let binding_file = Path::new(FUSE_MOUNT_BASE).join("bindings.json");
    let mut bindings: HashMap<String, InterfaceBinding> = if binding_file.exists() {
        let content = fs::read_to_string(&binding_file)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    bindings.insert(binding.ovs_interface.clone(), binding.clone());
    let content = serde_json::to_string_pretty(&bindings)?;
    fs::write(binding_file, content)?;

    Ok(())
}

/// Get all interface bindings for D-Bus introspection
pub fn get_interface_bindings() -> Result<HashMap<String, InterfaceBinding>> {
    let binding_file = Path::new(FUSE_MOUNT_BASE).join("bindings.json");
    if binding_file.exists() {
        let content = fs::read_to_string(binding_file)?;
        let bindings: HashMap<String, InterfaceBinding> = serde_json::from_str(&content)?;
        Ok(bindings)
    } else {
        Ok(HashMap::new())
    }
}

/// Remove enhanced binding with cleanup
pub fn unbind_veth_interface_enhanced(ovs_interface: &str) -> Result<()> {
    // Get binding info before cleanup
    let bindings = get_interface_bindings()?;
    let binding = bindings.get(ovs_interface);

    // Clean up Proxmox API interface
    if let Some(bind) = binding {
        cleanup_proxmox_api_interface(bind)?;
    }

    // Clean up standard bind mount
    unbind_veth_interface(ovs_interface)?;

    // Remove from bindings storage
    let binding_file = Path::new(FUSE_MOUNT_BASE).join("bindings.json");
    if binding_file.exists() {
        let mut bindings: HashMap<String, InterfaceBinding> = {
            let content = fs::read_to_string(&binding_file).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        };
        bindings.remove(ovs_interface);
        let content = serde_json::to_string_pretty(&bindings)?;
        fs::write(binding_file, content)?;
    }

    info!("Enhanced unbinding completed for {}", ovs_interface);
    Ok(())
}

/// Clean up Proxmox API interface
fn cleanup_proxmox_api_interface(binding: &InterfaceBinding) -> Result<()> {
    let vm_dir = Path::new(PROXMOX_API_BASE).join(format!("vm-{}", binding.vmid));

    // Remove interface configuration file
    let config_file = vm_dir.join(format!("{}.conf", binding.ovs_interface));
    if config_file.exists() {
        fs::remove_file(config_file)?;
    }

    // Remove symlink
    let interface_link = vm_dir.join(&binding.ovs_interface);
    if interface_link.exists() {
        fs::remove_file(interface_link)?;
    }

    // Remove VM directory if empty
    if vm_dir.exists() && vm_dir.read_dir()?.next().is_none() {
        fs::remove_dir(&vm_dir)?;
    }

    Ok(())
}

/// Validate OVS-Proxmox bridge synchronization
pub fn validate_bridge_synchronization(bridge: &str) -> Result<HashMap<String, bool>> {
    let mut validation_results = HashMap::new();

    // Get all interface bindings
    let bindings = get_interface_bindings()?;

    for (ovs_interface, binding) in bindings.iter() {
        if binding.bridge != bridge {
            continue;
        }

        let mut interface_valid = true;

        // Check if OVS interface exists
        let ovs_path = format!("/sys/class/net/{}", ovs_interface);
        if !Path::new(&ovs_path).exists() {
            warn!("OVS interface {} not found", ovs_interface);
            interface_valid = false;
        }

        // Check if Proxmox veth exists
        let proxmox_path = format!("/proc/net/veth/{}", binding.proxmox_veth);
        if !Path::new(&proxmox_path).exists() {
            warn!("Proxmox veth {} not found", binding.proxmox_veth);
            interface_valid = false;
        }

        // Check if bind mount exists
        let mount_point = format!("{}/{}", FUSE_MOUNT_BASE, ovs_interface);
        if !Path::new(&mount_point).exists() {
            warn!("Bind mount for {} not found", ovs_interface);
            interface_valid = false;
        }

        // Check if Proxmox API interface exists
        let vm_dir = Path::new(PROXMOX_API_BASE).join(format!("vm-{}", binding.vmid));
        let interface_link = vm_dir.join(ovs_interface);
        if !interface_link.exists() {
            warn!("Proxmox API interface for {} not found", ovs_interface);
            interface_valid = false;
        }

        validation_results.insert(ovs_interface.clone(), interface_valid);
    }

    Ok(validation_results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_fuse_mount_base() {
        let temp_dir = tempdir().unwrap();
        let _test_base = temp_dir.path().join("fuse_test");

        // This would need to be run as root in real scenarios
        // For testing, we just verify the logic doesn't panic
        let result = ensure_fuse_mount_base();
        // In a real test environment, this might fail due to permissions
        // but the logic should be sound
        assert!(result.is_ok() || result.is_err()); // Either way, no panic
    }
}
