//! OVSDB FUSE Mount - Mounts OVSDB as filesystem
//!
//! Usage: ovsdb-fuse-mount <mountpoint>

use anyhow::Result;
use ovs_port_agent::ovsdb_fuse::OvsdbFuse;
use std::env;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .init();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <mountpoint>", args[0]);
        eprintln!("Example: {} /var/lib/ovsdb-fuse", args[0]);
        std::process::exit(1);
    }
    
    let mountpoint = &args[1];
    
    println!("[1/6] Starting OVSDB FUSE mount");
    println!("      Mountpoint: {}", mountpoint);
    
    println!("[2/6] Checking mountpoint exists...");
    if !std::path::Path::new(mountpoint).exists() {
        eprintln!("ERROR: Mountpoint {} does not exist", mountpoint);
        eprintln!("Create it with: mkdir -p {}", mountpoint);
        std::process::exit(1);
    }
    println!("      ✓ Mountpoint exists");
    
    println!("[3/6] Creating OVSDB FUSE filesystem...");
    let fs = OvsdbFuse::new();
    println!("      ✓ Filesystem created");
    
    println!("[4/6] Loading real OVSDB data...");
    fs.load_real_data().expect("Failed to load OVSDB data");
    println!("      ✓ Real OVSDB data loaded");
    
    println!("[5/6] Preparing mount options...");
    let options = vec![
        fuser::MountOption::FSName("ovsdb".to_string()),
        fuser::MountOption::RO,
        fuser::MountOption::AllowOther,
    ];
    println!("      ✓ Options: read-only, allow-other");
    
    println!("[6/6] Mounting filesystem...");
    println!("      This will block - filesystem is now active");
    println!();
    println!("Filesystem structure:");
    println!("  {}/by-uuid/bridges/  - Bridges by UUID", mountpoint);
    println!("  {}/by-name/bridges/  - Bridges by name (symlinks)", mountpoint);
    println!("  {}/aliases/          - User-defined aliases", mountpoint);
    println!();
    println!("Press Ctrl+C to unmount");
    println!();
    
    match fuser::mount2(fs, mountpoint, &options) {
        Ok(_) => {
            println!("Filesystem unmounted cleanly");
            Ok(())
        }
        Err(e) => {
            eprintln!("ERROR: Failed to mount filesystem: {}", e);
            eprintln!();
            eprintln!("Common issues:");
            eprintln!("  - Mountpoint already in use: fusermount -u {}", mountpoint);
            eprintln!("  - Permission denied: Run as root");
            eprintln!("  - FUSE not available: modprobe fuse");
            Err(e.into())
        }
    }
}
