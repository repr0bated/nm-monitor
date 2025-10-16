//! OVSDB FUSE Mount - Mounts OVSDB as filesystem
//!
//! Usage: ovsdb-fuse-mount <mountpoint>

use anyhow::Result;
use ovs_port_agent::ovsdb_fuse::OvsdbFuse;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <mountpoint>", args[0]);
        eprintln!("Example: {} /var/lib/ovsdb-fuse", args[0]);
        std::process::exit(1);
    }
    
    let mountpoint = &args[1];
    
    println!("Mounting OVSDB filesystem at {}", mountpoint);
    println!("Structure:");
    println!("  /by-uuid/bridges/  - Bridges by UUID");
    println!("  /by-name/bridges/  - Bridges by name (symlinks)");
    println!("  /aliases/          - User-defined aliases");
    
    let fs = OvsdbFuse::new();
    
    // Initialize with sample data for testing
    fs.init_sample_data();
    
    let options = vec![
        fuser::MountOption::FSName("ovsdb".to_string()),
        fuser::MountOption::RO,
        fuser::MountOption::AllowOther,
    ];
    
    fuser::mount2(fs, mountpoint, &options)?;
    
    Ok(())
}
