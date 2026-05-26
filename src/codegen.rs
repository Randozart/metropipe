use std::fs;
use std::path::Path;

/// Generate all metropipe client stubs for a service into a directory.
/// Generate all language stubs for a service into a directory.
pub fn generate_stubs_to_dir(service_name: &str, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("mkdir failed: {}", e))?;

    generate_c_stub(service_name, out_dir)?;
    generate_python_stub(service_name, out_dir)?;
    generate_js_stub(service_name, out_dir)?;
    generate_rust_stub(service_name, out_dir)?;
    generate_go_stub(service_name, out_dir)?;
    generate_java_stub(service_name, out_dir)?;
    generate_csharp_stub(service_name, out_dir)?;
    generate_ruby_stub(service_name, out_dir)?;
    generate_bash_stub(service_name, out_dir)?;

    Ok(())
}

/// Entry point for `metropipe bind <library>` — generates .dbv + stubs
/// Entry point for `metropipe bind`.
pub fn generate_all_stubs(lib_name: &str, out_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(out_dir);
    fs::create_dir_all(dir)?;

    // Generate .dbv service definition
    let dbv = format!(
        "SERVICE {} {{\n  INPUT request: Data;\n  OUTPUT response: Data;\n}}\n",
        lib_name
    );
    fs::write(dir.join("service.dbv"), &dbv)?;

    // Generate client stubs
    generate_stubs_to_dir(lib_name, dir)
        .map_err(|e| format!("stub generation: {}", e))?;

    println!("Generated stubs for '{}' in {}", lib_name, out_dir);
    Ok(())
}

/// Write a file to the output directory.
fn write_file(dir: &Path, name: &str, content: &str) -> Result<(), String> {
    fs::write(dir.join(name), content)
        .map_err(|e| format!("write {} failed: {}", name, e))
}

/// Generate C header stub for a service.
fn generate_c_stub(name: &str, dir: &Path) -> Result<(), String> {
    let h = format!(r#"
#ifndef METROPIPE_{0}_H
#define METROPIPE_{0}_H

#include <stdint.h>
#include <stdatomic.h>
#include <stddef.h>

#define METRO_SERVICE "{1}"
#define METRO_SHM_PATH "/dev/shm/metro_{1}"
#define METRO_HEADER_SIZE 32
#define METRO_OFFSET_PAYLOAD 32
#define METRO_CAPACITY 4096

#define METRO_STATUS_IDLE 0
#define METRO_STATUS_CONSUMER_REQ 1
#define METRO_STATUS_PROVIDER_RES 3

typedef struct {{
    volatile uint32_t *header;
    volatile uint8_t  *payload;
    size_t capacity;
    int fd;
}} MetroChannel;

int metro_channel_open(MetroChannel *ch, const char *shm_path);
int metro_channel_send(MetroChannel *ch, const uint8_t *data, size_t len);
int metro_channel_recv(MetroChannel *ch, uint8_t *out, size_t max_len, int timeout_ms);

#endif
"#, name.to_uppercase(), name);
    write_file(dir, &format!("metropipe_{}.h", name), &h)
}

/// Generate Python stub for a service.
fn generate_python_stub(name: &str, dir: &Path) -> Result<(), String> {
    let py = format!(r#"
import mmap, struct, time, os

SHM_PATH = "/dev/shm/metro_{name}"
STATUS_IDLE = 0
STATUS_CONSUMER_REQ = 1
STATUS_PROVIDER_RES = 3
PAYLOAD_OFFSET = 32

class {Name}Client:
    def __init__(self):
        fd = open(SHM_PATH, "r+b")
        self.mm = mmap.mmap(fd.fileno(), 0)

    def request(self, payload: bytes, timeout_ms=5000) -> bytes:
        start = time.monotonic()
        while struct.unpack_from("<I", self.mm, 0)[0] != STATUS_IDLE:
            if (time.monotonic() - start) * 1000 > timeout_ms:
                raise TimeoutError("timeout")
            time.sleep(0.001)
        self.mm[PAYLOAD_OFFSET:PAYLOAD_OFFSET+len(payload)] = payload
        struct.pack_into("<I", self.mm, 8, len(payload))
        struct.pack_into("<I", self.mm, 0, STATUS_CONSUMER_REQ)
        resp_start = time.monotonic()
        while True:
            status = struct.unpack_from("<I", self.mm, 0)[0]
            if status == STATUS_PROVIDER_RES:
                size = struct.unpack_from("<I", self.mm, 8)[0]
                resp = bytes(self.mm[PAYLOAD_OFFSET:PAYLOAD_OFFSET+size])
                struct.pack_into("<I", self.mm, 0, STATUS_IDLE)
                return resp
            if (time.monotonic() - resp_start) * 1000 > timeout_ms:
                raise TimeoutError("timeout")
            time.sleep(0.001)

    def close(self):
        self.mm.close()
"#, name = name, Name = format!("{}{}", name[..1].to_uppercase(), &name[1..]));
    write_file(dir, &format!("metropipe_{}.py", name), &py)
}

/// Generate JavaScript stub for a service.
fn generate_js_stub(name: &str, dir: &Path) -> Result<(), String> {
    let js = format!(r#"
const fs = require('fs');
const path = require('path');

const SHM_PATH = "/dev/shm/metro_{name}";
const STATUS_IDLE = 0;
const STATUS_CONSUMER_REQ = 1;
const STATUS_PROVIDER_RES = 3;
const OFFSET_PAYLOAD = 32;

class {Name}Client {{
    constructor() {{
        const size = fs.statSync(SHM_PATH).size;
        this.buffer = new SharedArrayBuffer(size);
        this.header = new Int32Array(this.buffer, 0, 8);
        this.payload = new Uint8Array(this.buffer, OFFSET_PAYLOAD);
        new Uint8Array(this.buffer).set(fs.readFileSync(SHM_PATH));
    }}

    async request(payload, timeoutMs = 5000) {{
        const start = Date.now();
        while (Atomics.load(this.header, 0) !== STATUS_IDLE) {{
            if (Date.now() - start > timeoutMs) throw new Error('timeout');
            await new Promise(r => setTimeout(r, 1));
        }}
        this.payload.set(payload);
        this.header[2] = payload.length;
        Atomics.store(this.header, 0, STATUS_CONSUMER_REQ);
        const respStart = Date.now();
        while (true) {{
            const status = Atomics.load(this.header, 0);
            if (status === STATUS_PROVIDER_RES) {{
                const size = this.header[2];
                const resp = this.payload.slice(0, size);
                Atomics.store(this.header, 0, STATUS_IDLE);
                return resp;
            }}
            if (Date.now() - respStart > timeoutMs) throw new Error('timeout');
            await new Promise(r => setTimeout(r, 1));
        }}
    }}
}}

module.exports = {{ {Name}Client }};
"#, name = name, Name = format!("{}{}", name[..1].to_uppercase(), &name[1..]));
    write_file(dir, &format!("metropipe_{}.js", name), &js)
}

/// Generate Rust stub for a service.
fn generate_rust_stub(name: &str, dir: &Path) -> Result<(), String> {
    let rs = format!(r#"
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::Instant;

const SHM_PATH: &str = "/dev/shm/metro_{name}";
const STATUS_IDLE: u32 = 0;
const STATUS_CONSUMER_REQ: u32 = 1;
const STATUS_PROVIDER_RES: u32 = 3;
const PAYLOAD_OFFSET: u32 = 32;

pub struct {Name}Channel {{
    ptr: *mut u8,
    len: usize,
}}

impl {Name}Channel {{
    pub fn open() -> Result<Self, String> {{
        let fd = OpenOptions::new()
            .read(true).write(true)
            .open(SHM_PATH).map_err(|e| e.to_string())?;
        let len = std::fs::metadata(SHM_PATH).map_err(|e| e.to_string())?.len() as usize;
        let ptr = unsafe {{ libc::mmap(std::ptr::null_mut(), len,
            libc::PROT_READ|libc::PROT_WRITE, libc::MAP_SHARED, fd.as_raw_fd(), 0) }};
        if ptr == libc::MAP_FAILED {{ return Err("mmap".into()); }}
        Ok(Self {{ ptr: ptr as *mut u8, len }})
    }}

    pub fn request(&self, payload: &[u8], timeout_ms: u64) -> Result<Vec<u8>, String> {{
        let start = Instant::now();
        unsafe {{
            let status = &*(self.ptr as *const std::sync::atomic::AtomicU32);
            let size_ptr = &*(self.ptr.add(8) as *const std::sync::atomic::AtomicU32);
            let payload_ptr = self.ptr.add(PAYLOAD_OFFSET as usize);
            while status.load(std::sync::atomic::Ordering::SeqCst) != STATUS_IDLE {{
                if start.elapsed().as_millis() as u64 > timeout_ms {{
                    return Err("timeout".into());
                }}
            }}
            std::ptr::copy_nonoverlapping(payload.as_ptr(), payload_ptr, payload.len());
            size_ptr.store(payload.len() as u32, std::sync::atomic::Ordering::SeqCst);
            status.store(STATUS_CONSUMER_REQ, std::sync::atomic::Ordering::SeqCst);
            let resp_start = Instant::now();
            loop {{
                let s = status.load(std::sync::atomic::Ordering::SeqCst);
                if s == STATUS_PROVIDER_RES {{
                    let resp_size = size_ptr.load(std::sync::atomic::Ordering::SeqCst) as usize;
                    let mut resp = vec![0u8; resp_size];
                    std::ptr::copy_nonoverlapping(payload_ptr, resp.as_mut_ptr(), resp_size);
                    status.store(STATUS_IDLE, std::sync::atomic::Ordering::SeqCst);
                    return Ok(resp);
                }}
                if resp_start.elapsed().as_millis() as u64 > timeout_ms {{
                    return Err("timeout".into());
                }}
            }}
        }}
    }}
}}
"#, name = name, Name = format!("{}{}", name[..1].to_uppercase(), &name[1..]));
    write_file(dir, &format!("metropipe_{}.rs", name), &rs)
}

/// Generate Go stub for a service.
fn generate_go_stub(name: &str, dir: &Path) -> Result<(), String> {
    let go = format!(r#"
package {name}

import (
    "os" "syscall" "time" "encoding/binary"
)

const shmPath = "/dev/shm/metro_{name}"
const statusIdle = 0
const statusConsumerReq = 1
const statusProviderRes = 3
const payloadOffset = 32

type {Name}Channel struct {{ data []byte }}

func Open() (*{Name}Channel, error) {{
    f, err := os.OpenFile(shmPath, os.O_RDWR, 0)
    if err != nil {{ return nil, err }}
    defer f.Close()
    fi, _ := f.Stat()
    data, err := syscall.Mmap(int(f.Fd()), 0, int(fi.Size()),
        syscall.PROT_READ|syscall.PROT_WRITE, syscall.MAP_SHARED)
    if err != nil {{ return nil, err }}
    return &{Name}Channel{{data: data}}, nil
}}

func (ch *{Name}Channel) Request(payload []byte, timeoutMs int) ([]byte, error) {{
    start := time.Now()
    for binary.LittleEndian.Uint32(ch.data[0:4]) != statusIdle {{
        if time.Since(start).Milliseconds() > int64(timeoutMs) {{ return nil, fmt.Errorf("timeout") }}
        time.Sleep(time.Millisecond)
    }}
    copy(ch.data[payloadOffset:payloadOffset+len(payload)], payload)
    binary.LittleEndian.PutUint32(ch.data[8:12], uint32(len(payload)))
    binary.LittleEndian.PutUint32(ch.data[0:4], statusConsumerReq)
    respStart := time.Now()
    for {{
        s := binary.LittleEndian.Uint32(ch.data[0:4])
        if s == statusProviderRes {{
            sz := binary.LittleEndian.Uint32(ch.data[8:12])
            resp := make([]byte, sz)
            copy(resp, ch.data[payloadOffset:payloadOffset+int(sz)])
            binary.LittleEndian.PutUint32(ch.data[0:4], statusIdle)
            return resp, nil
        }}
        if time.Since(respStart).Milliseconds() > int64(timeoutMs) {{ return nil, fmt.Errorf("timeout") }}
        time.Sleep(time.Millisecond)
    }}
}}
"#, name = name, Name = format!("{}{}", name[..1].to_uppercase(), &name[1..]));
    write_file(dir, &format!("metropipe_{}.go", name), &go)
}

/// Generate Java stub for a service.
fn generate_java_stub(name: &str, dir: &Path) -> Result<(), String> {
    let Name = format!("{}{}", name[0..1].to_uppercase(), &name[1..]);
    let java = format!(r#"
import java.io.RandomAccessFile;
import java.nio.MappedByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.ByteOrder;

public class {Name}Channel {{
    private static final String SHM_PATH = "/dev/shm/metro_{name}";
    private static final int PAYLOAD_OFFSET = 32;
    private static final int STATUS_IDLE = 0;
    private static final int STATUS_CONSUMER_REQ = 1;
    private static final int STATUS_PROVIDER_RES = 3;
    private MappedByteBuffer buf;

    public {Name}Channel() throws Exception {{
        RandomAccessFile f = new RandomAccessFile(SHM_PATH, "rw");
        this.buf = f.getChannel().map(FileChannel.MapMode.READ_WRITE, 0, 32 + 4096);
        this.buf.order(ByteOrder.LITTLE_ENDIAN);
    }}

    public byte[] request(byte[] payload, long timeoutMs) throws Exception {{
        long start = System.nanoTime();
        while (buf.getInt(0) != STATUS_IDLE) {{
            if ((System.nanoTime() - start) / 1_000_000 > timeoutMs) throw new Exception("timeout");
            Thread.sleep(1);
        }}
        buf.position(PAYLOAD_OFFSET); buf.put(payload);
        buf.putInt(8, payload.length); buf.putInt(0, STATUS_CONSUMER_REQ);
        long respStart = System.nanoTime();
        while (true) {{
            int s = buf.getInt(0);
            if (s == STATUS_PROVIDER_RES) {{
                int sz = buf.getInt(8); byte[] resp = new byte[sz];
                buf.position(PAYLOAD_OFFSET); buf.get(resp, 0, sz);
                buf.putInt(0, STATUS_IDLE); return resp;
            }}
            if ((System.nanoTime() - respStart) / 1_000_000 > timeoutMs) throw new Exception("timeout");
            Thread.sleep(1);
        }}
    }}
}}
"#);
    write_file(dir, &format!("{Name}Channel.java", Name = Name), &java)
}

/// Generate C# stub for a service.
fn generate_csharp_stub(name: &str, dir: &Path) -> Result<(), String> {
    let Name = format!("{}{}", name[0..1].to_uppercase(), &name[1..]);
    let cs = format!(r#"
using System;
using System.IO.MemoryMappedFiles;
using System.Threading;

class {Name}Channel {{
    const string ShmPath = "/dev/shm/metro_{name}";
    const int PayloadOffset = 32;
    const int StatusIdle = 0;
    const int StatusConsumerReq = 1;
    const int StatusProviderRes = 3;
    private MemoryMappedViewAccessor acc;

    public {Name}Channel() {{
        var mmf = MemoryMappedFile.CreateFromFile(ShmPath, FileMode.Open);
        acc = mmf.CreateViewAccessor(0, 32 + 4096);
    }}

    public byte[] Request(byte[] payload, int timeoutMs) {{
        var start = DateTime.Now;
        while (acc.ReadInt32(0) != StatusIdle) {{
            if ((DateTime.Now - start).TotalMilliseconds > timeoutMs) throw new TimeoutException();
            Thread.Sleep(1);
        }}
        acc.WriteArray(PayloadOffset, payload, 0, payload.Length);
        acc.Write(8, payload.Length); acc.Write(0, StatusConsumerReq);
        var rs = DateTime.Now;
        while (true) {{
            int s = acc.ReadInt32(0);
            if (s == StatusProviderRes) {{
                int sz = acc.ReadInt32(8); byte[] resp = new byte[sz];
                acc.ReadArray(PayloadOffset, resp, 0, sz);
                acc.Write(0, StatusIdle); return resp;
            }}
            if ((DateTime.Now - rs).TotalMilliseconds > timeoutMs) throw new TimeoutException();
            Thread.Sleep(1);
        }}
    }}
}}
"#);
    write_file(dir, &format!("{Name}Channel.cs", Name = Name), &cs)
}

/// Generate Ruby stub for a service.
fn generate_ruby_stub(name: &str, dir: &Path) -> Result<(), String> {
    let rb = format!(r#"
require 'io/extra'

SHM_PATH = "/dev/shm/metro_{name}"
STATUS_IDLE = 0
STATUS_CONSUMER_REQ = 1
STATUS_PROVIDER_RES = 3
PAYLOAD_OFFSET = 32

class {Name}Client
  def initialize
    @fd = IO.sysopen(SHM_PATH, File::RDWR)
    @buf = IO.mmap(@fd, File.size(SHM_PATH), IO::PROT_READ|IO::PROT_WRITE, IO::MAP_SHARED)
  end

  def request(payload, timeout_ms = 5000)
    start = Time.now
    while @buf[0, 4].unpack1('L') != STATUS_IDLE
      raise 'timeout' if (Time.now - start) * 1000 > timeout_ms
      sleep 0.001
    end
    @buf[PAYLOAD_OFFSET, payload.bytesize] = payload
    @buf[8, 4] = [payload.bytesize].pack('L')
    @buf[0, 4] = [STATUS_CONSUMER_REQ].pack('L')
    rs = Time.now
    loop do
      s = @buf[0, 4].unpack1('L')
      if s == STATUS_PROVIDER_RES
        sz = @buf[8, 4].unpack1('L')
        resp = @buf[PAYLOAD_OFFSET, sz]
        @buf[0, 4] = [STATUS_IDLE].pack('L')
        return resp
      end
      raise 'timeout' if (Time.now - rs) * 1000 > timeout_ms
      sleep 0.001
    end
  end

  def close
    IO.munmap(@buf); @fd.close
  end
end
"#, name = name, Name = format!("{}{}", name[..1].to_uppercase(), &name[1..]));
    write_file(dir, &format!("metropipe_{}.rb", name), &rb)
}

/// Generate Bash stub for a service.
/// Generate Bash stub for a service (uses metropipe proxy).
fn generate_bash_stub(name: &str, dir: &Path) -> Result<(), String> {
    let sh = format!(r#"#!/bin/bash
# metropipe bash stub for {name}
# Usage: source metropipe_{name}.sh && metropipe_request "payload"
# Requires: metropipe proxy (handles the mmap)

METROPIPE_SERVICE="{name}"

metropipe_request() {{
    echo "$1" | metropipe proxy "$METROPIPE_SERVICE"
}}
"#, name = name);
    write_file(dir, &format!("metropipe_{}.sh", name), &sh)
}
