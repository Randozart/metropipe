# metropipe

Share data between processes on the same machine using shared memory.

Write to a buffer in one language, read it from another. Works with C, Go, Python, Java, Rust, JavaScript, C#, Ruby, Bash, and anything else that can open a file or read stdin.

## Install

```bash
cargo install metropipe
```

Or download a pre-built binary from the [releases page](https://github.com/Randozart/metropipe/releases).

## Commands

### `metropipe serve` — start the daemon

Allocates shared memory channels in `/dev/shm/` and waits for clients.

```bash
metropipe serve
```

### `metropipe connect <name>` — interactive REPL

Opens a channel, reads lines from stdin, sends each as a request, prints the response.

```bash
metropipe connect WeatherApi
Connected to /dev/shm/metro_WeatherApi
> New York
Response: sunny, 72°F
```

Flags:
- `--send <data>` — one-shot: send once, print response, exit
- `--listen` — act as the provider: receive requests, prompt for responses
- `--gen-stubs [<dir>]` — generate client library files for 9 languages

### `metropipe bind <library>` — generate client stubs

Analyzes a library and generates `.dbv` + client stubs for all supported languages.

```bash
metropipe bind mylib.h
Generated stubs for 'mylib' in lib/ffi/generated/mylib/
```

### `metropipe proxy <name>` — stdin/stdout bridge

Wraps the shared memory handshake as a pipe. Any language that can read stdin and write stdout can participate.

```bash
echo "payload" | metropipe proxy WeatherApi > response.bin
```

Useful for: Bash scripts, AWK, Perl, PHP, Lua — anything without `mmap`.

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

1. `metropipe serve` creates a file at `/dev/shm/metro_<name>` with the 32-byte header + zero-filled payload.
2. A client opens the file, memory-maps it, and writes a request to the payload region.
3. The client sets STATUS_WORD to `1` (CONSUMER_REQ) via an atomic store.
4. The provider polls STATUS_WORD. When it sees `1`, it processes the request and writes back.
5. The provider sets STATUS_WORD to `3` (PROVIDER_RES) when done.
6. The client reads the response and resets STATUS_WORD to `0` (IDLE).

The same buffer, header, and handshake work for every language. No serialization, no function calls, no `.so` files.

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
