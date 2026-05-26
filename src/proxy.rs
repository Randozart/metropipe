use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::channel;

/// Run the metropipe stdin/stdout bridge for non-mmap languages.
///
/// Reads lines from stdin, sends each as a request via shared memory,
/// and writes responses to stdout. Any language with text I/O can participate.
pub fn run_proxy(service_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let shm_path = format!("/dev/shm/metro_{}", service_name);

    // Allocate channel if it doesn't exist
    if !Path::new(&shm_path).exists() {
        eprintln!("Note: allocating {}", shm_path);
        let fd = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&shm_path)?;
        fd.set_len((channel::HEADER_SIZE + channel::DEFAULT_CAPACITY) as u64)?;
    }

    // Open and mmap
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&shm_path)?;
    let file_len = file.metadata()?.len() as usize;
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            file_len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            file.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err("mmap failed".into());
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, file_len) };

    // Process stdin lines
    for line in std::io::stdin().lines() {
        let line = line?;
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        match channel::request(buf, line.as_bytes(), 5000) {
            Ok(resp) => {
                std::io::stdout().write_all(&resp)?;
                std::io::stdout().write_all(b"\n")?;
            }
            Err(e) => {
                eprintln!("proxy error: {}", e);
            }
        }
    }

    unsafe { libc::munmap(ptr, file_len); }
    Ok(())
}
