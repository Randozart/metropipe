# metropipe

Pass data between processes on the same machine using shared memory. Works on Linux, macOS, Docker, WSL — everywhere.

Write to a buffer in one language, read it from another. No server, no daemon, no setup.

## Install

```bash
cargo install metropipe
```

Or download a [pre-built binary](https://github.com/Randozart/metropipe/releases).

## Commands

### `export` — Make a function callable from any language

```bash
# Read classify() from services.py, generate RPC stubs in Rust and Go
metropipe export classify services.py --target rust go

# Run the provider — wraps your real function in a poll loop
python3 metropipe/classify/provider.py &

# Call from Rust (using the generated stub)
let (label, conf) = classify("image.jpg", 3)?;
```

Output modes:

| Flag | Pattern | Example |
|------|---------|---------|
| `--namespace` (default) | `metropipe/classify/stub.rs` | One directory per function |
| `--flat` | `metropipe/classify.rs` | All in one directory |
| `--unify` | `metropipe.rs` | All functions merged into one file per language |

Without `--target`, generates stubs for all 9 languages. Source language is detected from the file extension (`.py`, `.rs`, `.go`, `.c`, `.js`, `.ts`, `.rb`, `.java`, `.cs`). Unknown extensions produce raw bytes stubs.

### `connect` — Talk to a service from the terminal

```bash
metropipe connect WeatherApi        # Interactive REPL
metropipe connect WeatherApi --send "London"     # One-shot
metropipe connect WeatherApi --listen            # Act as provider
metropipe connect WeatherApi --gen-stubs         # Generate client stubs
```

The channel is created automatically on first use.

### `proxy` — stdin/stdout bridge for languages without mmap

```bash
echo "New York" | metropipe proxy WeatherApi > response.bin
```

Works with Bash, AWK, Perl, PHP, Lua — anything that can read stdin.

### `bind` — Generate stubs from a library file

```bash
metropipe bind mylib.h
```

## How it works

1. The first process to use a channel creates a file at `/dev/shm/metro_<name>` (or `/tmp/` on macOS, or `./.metropipe/` as fallback). Set `$METROPIPE_DIR` to override.
2. Processes open + mmap the file and exchange data through a 32-byte header + payload area.
3. Consumer writes data, sets status word to `CONSUMER_REQ`. Provider polls, sees the request, processes it, writes the response, sets status word to `PROVIDER_RES`. Consumer reads and resets to `IDLE`.
4. The file is the channel. No server, no central registry, no setup.

## Language Support

| Language | Generated stub | How it connects |
|----------|---------------|-----------------|
| C | `metropipe/<fn>/stub.h` | mmap + atomic ops |
| Go | `metropipe/<fn>/stub.go` | syscall.Mmap |
| Python | `metropipe/<fn>/stub.py` | mmap |
| Java | `metropipe/<fn>/stub.java` | MappedByteBuffer |
| Rust | `metropipe/<fn>/stub.rs` | mmap + libc |
| C# | `metropipe/<fn>/stub.cs` | MemoryMappedFile |
| JavaScript | `metropipe/<fn>/stub.js` | SharedArrayBuffer |
| Ruby | `metropipe/<fn>/stub.rb` | IO.mmap |
| Bash | `metropipe/<fn>/stub.sh` | metropipe proxy |

## Protocol

All channels use a 32-byte header:

| Offset | Size | Field | Values |
|--------|------|-------|--------|
| 0 | 4 | STATUS_WORD | 0=idle, 1=request, 3=response, 4=error |
| 4 | 4 | CAS_LOCK | atomic mutex |
| 8 | 4 | PAYLOAD_SIZE | bytes written |
| 12 | 4 | MAX_CAPACITY | max payload |
| 16 | 4 | ERROR_CODE | error detail |
| 20 | 12 | (reserved) | padding |
| 32 | variable | PAYLOAD | data |

## Project Structure

```
metropipe/
├── src/
│   ├── main.rs       # CLI: export, connect, bind, proxy
│   ├── export.rs     # Function parsing + stub generation
│   ├── channel.rs    # 32-byte header protocol
│   ├── connect.rs    # REPL / send / listen / gen-stubs
│   ├── codegen.rs    # Multi-language stub generator
│   └── proxy.rs      # stdin/stdout bridge
├── Cargo.toml
└── docs/METROPOLITAN-SPEC.md
```

## Related

- [Brief Language](https://github.com/Randozart/brief-lang) — optional, for contract-verified builds
