# metropipe

Pass data between processes on the same machine using shared memory. Works on Linux, macOS, Docker, WSL — everywhere.

Write to a buffer in one language, read it from another. No server, no daemon, no setup.

## Install

```bash
cargo install metropipe
```

Or download a [pre-built binary](https://github.com/Randozart/metropipe/releases).

## Usage

```bash
# Connect to a service — creates the channel if it doesn't exist
metropipe connect WeatherApi
> New York
Response: sunny, 72°F

# One-shot mode
metropipe connect WeatherApi --send "London"

# Act as the provider (receive and respond to requests)
metropipe connect WeatherApi --listen

# Generate client stubs for 9 languages
metropipe connect WeatherApi --gen-stubs

# stdin/stdout bridge — for languages without mmap
echo "New York" | metropipe proxy WeatherApi > response.bin

# Generate stubs from a library file
metropipe bind mylib.h
```

## How it works

1. The first process to use a channel creates a file at `/dev/shm/metro_<name>` (or `/tmp/metro_<name>` on macOS, or `./.metropipe/metro_<name>` as fallback).
2. Processes open + mmap the file and exchange data through a 32-byte header + payload area.
3. A consumer writes data, sets STATUS to CONSUMER_REQ, the provider sees it and responds, sets STATUS to PROVIDER_RES, consumer reads and resets to IDLE.
4. The file itself IS the channel. No server, no central registry. Set `$METROPIPE_DIR` to override the storage directory.

The same file, same header layout, same atomic handshake works in every language.

## Language Support

| Language | File | How it connects |
|----------|------|----------------|
| C | `metropipe_<svc>.h` | mmap + atomic ops |
| Go | `metropipe_<svc>.go` | syscall.Mmap |
| Python | `metropipe_<svc>.py` | mmap |
| Java | `metropipe_<svc>.java` | MappedByteBuffer |
| Rust | `metropipe_<svc>.rs` | mmap + libc |
| C# | `metropipe_<svc>.cs` | MemoryMappedFile |
| JavaScript | `metropipe_<svc>.js` | SharedArrayBuffer |
| Ruby | `metropipe_<svc>.rb` | IO.mmap |
| Bash | `metropipe_<svc>.sh` | metropipe proxy |

Generate stubs for any service:
```bash
metropipe connect WeatherApi --gen-stubs ./my_stubs
```

## Protocol

All channels use a 32-byte header at the start of the shared memory file:

| Offset | Size | Field | Meaning |
|--------|------|-------|---------|
| 0 | 4 | STATUS_WORD | 0=idle, 1=request, 3=response, 4=error |
| 4 | 4 | CAS_LOCK | atomic mutex |
| 8 | 4 | PAYLOAD_SIZE | bytes written |
| 12 | 4 | MAX_CAPACITY | max payload |
| 16 | 4 | ERROR_CODE | error detail |
| 20 | 12 | (reserved) | padding |
| 32 | variable | PAYLOAD | data |

Communication follows: idle → consumer writes → consumer signals request → provider processes → provider signals response → consumer reads → idle.

## How it works

1. The first process to connect creates a file at `/dev/shm/metro_<name>` with the 32-byte header + zero-filled payload.
2. A client opens the file, memory-maps it, and writes a request to the payload region.
3. The client sets STATUS_WORD to `1` (CONSUMER_REQ) via an atomic store.
4. The provider polls STATUS_WORD. When it sees `1`, it processes the request and writes back.
5. The provider sets STATUS_WORD to `3` (PROVIDER_RES) when done.
6. The client reads the response and resets STATUS_WORD to `0` (IDLE).

No daemon, no central registry, no server process. The file path `/dev/shm/metro_<name>` IS the channel — any process that knows the name can join by opening and mmap-ing the same file.

## Project Structure

```
metropipe/
├── src/
│   ├── main.rs            # CLI entry point
│   ├── channel.rs         # 32-byte header protocol
│   ├── server.rs          # daemon
│   ├── connect.rs         # REPL / send / listen / gen-stubs
│   ├── proxy.rs           # stdin/stdout bridge
│   └── codegen.rs         # stub generator (9 languages)
├── clients/               # reference client implementations
├── docs/METROPOLITAN-SPEC.md
└── Cargo.toml
```

## Related

- [Brief Language](https://github.com/Randozart/brief-lang) — optional, for contract-verified daemon builds
- [docs/METROPOLITAN-SPEC.md](docs/METROPOLITAN-SPEC.md) — protocol specification
