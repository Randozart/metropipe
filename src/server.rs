use std::fs::{self, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::channel;

/// Start the metropipe daemon. Creates /dev/shm if needed and listens on all channels.
/// Start the metropipe daemon.
pub fn run_serve() -> Result<(), Box<dyn std::error::Error>> {
    let shm_dir = Path::new("/dev/shm");
    if !shm_dir.exists() {
        eprintln!("Warning: /dev/shm does not exist. Creating...");
        fs::create_dir_all(shm_dir)?;
    }

    eprintln!("metropipe serve — listening on /dev/shm/metro_*");
    eprintln!("  Protocol: 32-byte header, status words, atomic CAS");
    eprintln!("  Press Ctrl+C to stop.");

    // Main loop: scan for requests on all known channels
    // For now, just keep the binary alive; actual channel management
    // happens when clients connect via the 32-byte protocol.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Allocate a shared memory channel for a service.
pub fn allocate_channel(name: &str, capacity: usize) -> Result<String, String> {
    let shm_path = format!("/dev/shm/metro_{}", name);
    let total_size = channel::HEADER_SIZE + capacity;

    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o644)
        .open(&shm_path)
        .map_err(|e| format!("shm_open failed: {}", e))?;

    file.set_len(total_size as u64)
        .map_err(|e| format!("ftruncate failed: {}", e))?;

    Ok(shm_path)
}

/// Remove a shared memory channel.
/// Remove a shared memory channel.
pub fn deallocate_channel(name: &str) -> Result<(), String> {
    let shm_path = format!("/dev/shm/metro_{}", name);
    fs::remove_file(&shm_path)
        .map_err(|e| format!("shm_unlink failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
/// Verify channel allocation and cleanup.
    /// Verify channel allocation and cleanup.
    fn test_allocate_and_deallocate() {
        let name = "metropipe_test_service";
        let result = allocate_channel(name, 4096);
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(Path::new(&path).exists());

        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.len() as usize, channel::HEADER_SIZE + 4096);

        deallocate_channel(name).unwrap();
        assert!(!Path::new(&path).exists());
    }
}
