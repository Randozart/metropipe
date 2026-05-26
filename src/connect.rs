use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::channel;

/// Run the metropipe connect CLI.
///
/// Opens a shared memory channel and performs the requested action:
/// - Default: interactive REPL (read lines from stdin, send as payload, print response)
/// - `--send <payload>`: one-shot RPC
/// - `--listen`: act as a provider (receive requests, prompt for responses)
/// - `--gen-stubs [<dir>]`: generate client stub files
/// Run the metropipe connect CLI.
pub fn run_connect(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let service_name = &args[0];
    let shm_path = format!("/dev/shm/metro_{}", service_name);

    // Parse flags
    let mut send_payload: Option<String> = None;
    let mut listen_mode = false;
    let mut gen_stubs = false;
    let mut stubs_dir = String::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--send" if i + 1 < args.len() => {
                send_payload = Some(args[i + 1].clone());
                i += 2;
            }
            "--listen" => {
                listen_mode = true;
                i += 1;
            }
            "--gen-stubs" => {
                gen_stubs = true;
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    stubs_dir = args[i + 1].clone();
                    i += 2;
                } else {
                    stubs_dir = format!("metropipe_{}_stubs", service_name);
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    // Check if the shared memory exists; create it if not (like metropipe allocates)
    if !Path::new(&shm_path).exists() {
        eprintln!("Note: {} not found. Allocating...", shm_path);
        let fd = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&shm_path)?;
        fd.set_len((channel::HEADER_SIZE + channel::DEFAULT_CAPACITY) as u64)?;
        eprintln!("  Allocated {} bytes", channel::HEADER_SIZE + channel::DEFAULT_CAPACITY);
    }

    // Open and mmap the channel
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

    // Generate stubs if requested
    if gen_stubs {
        let dir = Path::new(&stubs_dir);
        std::fs::create_dir_all(dir)?;
        crate::codegen::generate_stubs_to_dir(service_name, dir)?;
        println!("Stubs generated in {}", stubs_dir);
        return Ok(());
    }

    // Listen mode — act as a provider
    if listen_mode {
        eprintln!("Listening on {}...", shm_path);
        loop {
            let status = channel::read_status(buf);
            if status == channel::STATUS_CONSUMER_REQ {
                let req_size = channel::read_payload_size(buf) as usize;
                let req_data = buf[channel::OFFSET_PAYLOAD..channel::OFFSET_PAYLOAD + req_size].to_vec();
                let req_str = String::from_utf8_lossy(&req_data);
                println!("\nRequest: {}", req_str);
                println!("Enter response:");
                let mut resp_line = String::new();
                std::io::stdin().read_line(&mut resp_line)?;
                let resp = resp_line.trim().as_bytes().to_vec();
                let copy_len = resp.len().min(buf.len() - channel::OFFSET_PAYLOAD);
                buf[channel::OFFSET_PAYLOAD..channel::OFFSET_PAYLOAD + copy_len].copy_from_slice(&resp[..copy_len]);
                channel::write_payload_size(buf, copy_len as u32);
                channel::write_status(buf, channel::STATUS_PROVIDER_RES);
                eprintln!("Response sent ({} bytes)", copy_len);
            } else if status == channel::STATUS_ERROR {
                eprintln!("Error status received");
                channel::write_status(buf, channel::STATUS_IDLE);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    // One-shot RPC mode
    if let Some(payload) = send_payload {
        let resp = channel::request(buf, payload.as_bytes(), 5000)?;
        println!("{}", String::from_utf8_lossy(&resp));
        return Ok(());
    }

    // Interactive REPL mode
    println!("Connected to {}", shm_path);
    println!("Type a message and press Enter. Ctrl+C to quit.");
    for line in std::io::stdin().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match channel::request(buf, line.as_bytes(), 5000) {
            Ok(resp) => {
                let resp_str = String::from_utf8_lossy(&resp);
                println!("Response: {}", resp_str);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    unsafe { libc::munmap(ptr, file_len); }
    Ok(())
}
