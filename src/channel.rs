/// metropipe channel protocol — 32-byte header helpers.
///
/// All metropipe communications use the same memory layout regardless
/// of transport (mmap, pipe, or TCP):
///
/// | Offset | Field        | Description                     |
/// |--------|-------------|---------------------------------|
/// | 0x00   | STATUS_WORD  | 0=IDLE, 1=CONSUMER_REQ, ...    |
/// | 0x04   | CAS_LOCK     | Atomic compare-and-swap mutex   |
/// | 0x08   | PAYLOAD_SIZE | Bytes written in payload        |
/// | 0x0C   | MAX_CAPACITY | Maximum payload size            |
/// | 0x10   | ERROR_CODE   | Error metadata on ERROR status  |
/// | 0x14   | RESERVED     | 12 bytes padding                |
/// | 0x20   | PAYLOAD      | Raw byte data                   |

pub const HEADER_SIZE: usize = 32;
pub const OFFSET_STATUS: usize = 0;
pub const OFFSET_CAS_LOCK: usize = 4;
pub const OFFSET_PAYLOAD_SIZE: usize = 8;
pub const OFFSET_MAX_CAPACITY: usize = 12;
pub const OFFSET_ERROR_CODE: usize = 16;
pub const OFFSET_PAYLOAD: usize = 32;

pub const STATUS_IDLE: u32 = 0;
pub const STATUS_CONSUMER_REQ: u32 = 1;
pub const STATUS_PROVIDER_ACK: u32 = 2;
pub const STATUS_PROVIDER_RES: u32 = 3;
pub const STATUS_ERROR: u32 = 4;

/// Default capacity for new channels
pub const DEFAULT_CAPACITY: usize = 4096;

/// Read a little-endian u32 from a byte buffer at the given offset.
pub fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

/// Write a little-endian u32 to a byte buffer at the given offset.
pub fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

/// Read the status word from a metropipe buffer.
pub fn read_status(buf: &[u8]) -> u32 {
    read_u32(buf, OFFSET_STATUS)
}

/// Write the status word to a metropipe buffer.
pub fn write_status(buf: &mut [u8], status: u32) {
    write_u32(buf, OFFSET_STATUS, status)
}

/// Read the payload size from the metropipe header.
pub fn read_payload_size(buf: &[u8]) -> u32 {
    read_u32(buf, OFFSET_PAYLOAD_SIZE)
}

/// Write the payload size to the metropipe header.
pub fn write_payload_size(buf: &mut [u8], size: u32) {
    write_u32(buf, OFFSET_PAYLOAD_SIZE, size)
}

/// Read the error code from the metropipe header.
pub fn read_error_code(buf: &[u8]) -> u32 {
    read_u32(buf, OFFSET_ERROR_CODE)
}

/// Execute a full request/response cycle over a metropipe channel.
///
/// 1. Wait for IDLE status
/// 2. Write payload to the buffer
/// 3. Signal CONSUMER_REQ
/// 4. Poll for PROVIDER_RES (with timeout)
/// 5. Read response and reset to IDLE
pub fn request(buf: &mut [u8], payload: &[u8], timeout_ms: u64) -> Result<Vec<u8>, String> {
    let start = std::time::Instant::now();

    // Wait for IDLE
    while read_status(buf) != STATUS_IDLE {
        if start.elapsed().as_millis() as u64 > timeout_ms {
            return Err("timeout waiting for IDLE".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    // Write payload
    let payload_len = payload.len().min(buf.len() - OFFSET_PAYLOAD);
    buf[OFFSET_PAYLOAD..OFFSET_PAYLOAD + payload_len].copy_from_slice(&payload[..payload_len]);
    write_payload_size(buf, payload_len as u32);
    write_status(buf, STATUS_CONSUMER_REQ);

    // Poll for response
    let resp_start = std::time::Instant::now();
    loop {
        let status = read_status(buf);
        if status == STATUS_PROVIDER_RES {
            let resp_size = read_payload_size(buf) as usize;
            let mut resp = vec![0u8; resp_size];
            let copy_len = resp_size.min(buf.len() - OFFSET_PAYLOAD);
            resp[..copy_len].copy_from_slice(&buf[OFFSET_PAYLOAD..OFFSET_PAYLOAD + copy_len]);
            write_status(buf, STATUS_IDLE);
            return Ok(resp);
        }
        if status == STATUS_ERROR {
            let code = read_error_code(buf);
            write_status(buf, STATUS_IDLE);
            return Err(format!("provider error: code {}", code));
        }
        if resp_start.elapsed().as_millis() as u64 > timeout_ms {
            return Err("timeout waiting for response".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read_u32() {
        let mut buf = vec![0u8; 64];
        write_u32(&mut buf, 0, 42);
        assert_eq!(read_u32(&buf, 0), 42);
    }

    #[test]
    fn test_status_constants() {
        assert_eq!(STATUS_IDLE, 0);
        assert_eq!(STATUS_CONSUMER_REQ, 1);
        assert_eq!(STATUS_PROVIDER_RES, 3);
        assert_eq!(STATUS_ERROR, 4);
    }

    #[test]
    fn test_header_offsets() {
        assert_eq!(OFFSET_PAYLOAD, 32);
        assert_eq!(HEADER_SIZE, 32);
    }
}
