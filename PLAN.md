# metropipe — Universal Language Binder

**Weld any two languages together at the lowest level they're capable of welding.**

No C-ABI. No serialization. No wrappers. Just a block of shared memory and an atomic handshake — or a pipe, or a socket — depending on what the language can do.

## Philosophy

Every language has a lowest-common-denominator I/O capability. metropipe meets each language at that level:

| Level | Transport | Latency | Languages |
|-------|-----------|---------|-----------|
| **Binary** | Shared memory (mmap, SharedArrayBuffer, MMIO) | ~10ns | C, Rust, Go, Python, Java, C#, Ruby, JS/Node, Zig, Nim, OCaml, D, Swift |
| **Pipe** | stdin/stdout proxy | ~100μs | Bash, AWK, Perl, PHP, Lua, Tcl, Julia, R, Dart CLI, Elixir, Erlang, Haskell, OCaml |
| **Socket** | TCP relay (via `metropipe bridge`) | ~1ms | Everything with HTTP — curl, PowerShell, Kotlin, MATLAB, Flutter, Blazor, VBA |

Each level is a strict superset of the one above. A Bash script talks to a Rust binary talks to a Python service — all through the same `/dev/shm/metro_*` buffer at the bottom.

## The Protocol (32-byte Header)

All channels use the same layout regardless of transport:

| Offset | Field | Values |
|--------|-------|--------|
| 0x00 | STATUS_WORD | 0=IDLE, 1=CONSUMER_REQ, 2=PROVIDER_ACK, 3=PROVIDER_RES, 4=ERROR |
| 0x04 | CAS_LOCK | Atomic compare-and-swap mutex |
| 0x08 | PAYLOAD_SIZE | Bytes written in payload |
| 0x0C | MAX_CAPACITY | Maximum payload size |
| 0x10 | ERROR_CODE | Error metadata on ERROR |
| 0x14 | RESERVED | 12 bytes padding |
| 0x20 | PAYLOAD | Raw byte data |

Handshake: `IDLE → CONSUMER_REQ → PROVIDER_ACK → PROVIDER_RES → IDLE`

## The Goal: One Command

```bash
# Analyze a foreign library, generate stubs for every language
metropipe bind mylib.h
  → lib/ffi/generated/mylib/service.dbv
  → lib/ffi/generated/mylib/stub.c
  → lib/ffi/generated/mylib/stub.py
  → lib/ffi/generated/mylib/stub.js
  → lib/ffi/generated/mylib/stub.rs
  → lib/ffi/generated/mylib/stub.go
  → lib/ffi/generated/mylib/stub.java
  → lib/ffi/generated/mylib/stub.cs
  → lib/ffi/generated/mylib/stub.rb

# Connect from any language — even one without mmap
metropipe proxy WeatherApi
> New York
Response: temperature=72.5, humidity=0.45, condition="Sunny"

# Generate stubs for a specific service
metropipe connect WeatherApi --gen-stubs
  → metropipe_WeatherApi.h
  → metropipe_WeatherApi.py
  → metropipe_WeatherApi.js
  → metropipe_WeatherApi.rs
  → metropipe_WeatherApi.go

# Start the daemon (allocates shared memory)
metropipe serve

# Use from bash (no mmap needed):
echo "New York" | metropipe proxy WeatherApi > response.bin
```

## Architecture

```
┌──────────┐     ┌─────────────────────┐     ┌──────────┐
│  Python  │────→│                     │←────│  Node.js │
└──────────┘     │   /dev/shm/metro_*  │     └──────────┘
                 │   32-byte header    │
┌──────────┐     │   + payload plane   │     ┌──────────┐
│  Rust    │────→│   atomic CAS lock   │←────│  C/C++   │
└──────────┘     └──────────┬──────────┘     └──────────┘
                           │
                    ┌──────┴──────┐
                    │  metropipe  │
                    │   proxy    │←── stdin/stdout (bash, awk, perl, ...)
                    └─────────────┘
```

## Project Structure

```
metropipe/
├── Cargo.toml              # Standalone Rust binary (no brief-compiler dep)
├── src/
│   ├── main.rs             # CLI: serve, connect, bind, proxy
│   ├── server.rs           # Daemon: shm_open + reactive poll loop
│   ├── channel.rs          # 32-byte header protocol helpers
│   ├── connect.rs          # REPL, --send, --listen, --gen-stubs
│   ├── codegen.rs          # Stub generation for 9+ languages
│   ├── proxy.rs            # stdin/stdout bridge for non-mmap languages
│   └── bind.rs             # Analyze library → .dbv + stubs
├── clients/
│   ├── c/metropipe.h       # C header (mmap)
│   ├── python/metropipe.py # Python client (mmap)
│   ├── javascript/metropipe.js # Node.js client (SharedArrayBuffer)
│   ├── rust/metropipe.rs   # Rust module (mmap + libc)
│   ├── go/metropipe.go     # Go package (syscall.Mmap)
│   ├── java/MetroChannel.java  # Java class (MappedByteBuffer)
│   ├── csharp/MetroChannel.cs  # C# class (MemoryMappedFile)
│   └── ruby/metropipe.rb   # Ruby module (IO.mmap)
├── docs/METROPOLITAN-SPEC.md
├── PLAN.md
└── README.md
```

## Phases

### Phase R: Rename (✅ DONE)
`metrod` → `metropipe`. Folder, binary, docs, all references.

### Phase H: `metropipe bind` — Service Generation (in progress)
`metropipe bind mylib.h` emits `.dbv` IDL + client stubs for all languages.

Files: `src/bind.rs`, `src/codegen.rs`, `clients/*`

### Phase I: `metropipe connect` — Full RPC (2 days)
True 32-byte protocol REPL, `--send`, `--listen`, `--gen-stubs`.

Files: `src/connect.rs`

### Phase J: `metropipe serve` — Daemon (1 week)
Real shm_open/ftruncate/mmap via the daemon. Service registry. Hot-reload.

Files: `src/server.rs`, `metropipe.bv` (optional reference)

### Phase K: Brief Reference Implementation (3 days)
The daemon in Brief (`metropipe.bv`) — contract-verified version of the Rust daemon.
Proves the FFI cycle: Brief daemon → generated stubs → any language.

Files: `src/metropipe.bv`, `lib/std/metropipe_gen.bv`

### Phase L: `metropipe proxy` — stdin/stdout Bridge (1 day)
Wraps shared memory handshake as stdin/stdout. Every language with text I/O becomes a client.

```bash
echo "payload" | metropipe proxy WeatherApi > response.bin
```

Files: `src/proxy.rs`

### Phase S: Standalone Binary ✅
`cargo install metropipe` — zero dependencies. The brief-compiler becomes optional.

Files: `Cargo.toml`, `src/main.rs`

### Phase X: Cross-Platform Paths ✅
`resolve_shm_path()` tries `$METROPIPE_DIR`, `/dev/shm`, `/tmp`, `./.metropipe` — works everywhere.

### Phase E: `metropipe export <function> <source> [--target lang...] [--out <dir>] [--namespace|--flat|--unify]`

Read a function from a source file, generate typed stubs + provider in targeted languages.

#### Command
```
metropipe export classify services.py --target rust go c --out ./api
```

#### Arguments
- `<function>` — the function name to export
- `<source>` — path to the source file (extension determines language)
- `--target <lang>` — repeatable, one per target language (default: all 9)
- `--out <dir>` — output directory (default: `./metropipe`)

#### Output modes
- `--namespace` (default): `metropipe/classify/stub.rs`, `metropipe/classify/provider.py`
- `--flat`: `metropipe/classify.rs`, `metropipe/classify.py`
- `--unify`: `metropipe.rs`, `metropipe.py` (all exports merged)

#### Per-language function parsers
Extract name, parameter types, and return types from source:
- `.py` — `def fn(params) -> type:`
- `.rs` — `fn name(params) -> type`
- `.go` — `func Name(params) type`
- `.c` / `.h` — `type name(params)`
- `.js` / `.mjs` — `function name(params)`
- `.ts` — `function name(params): type`
- `.rb` — `def name(params)`
- `.java` — `type name(params)`
- `.cs` — `type Name(params)`

Unknown `.xyz` → raw bytes stub.

#### Provider generation
For each exported function, generate a provider script in the source language:
- Imports the real function from the source file
- Opens a metropipe channel named after the function
- Runs the poll loop: `wait_request()` → deserialize → call → serialize → `send_response()`

User runs: `python3 metropipe/classify/provider.py`

Files: `src/export.rs`, `src/codegen.rs` (extended)

## Welding Hierarchy — Complete

```
Language capable of mmap?
  ├── yes → use mmap directly (~10ns)
  │         C, Rust, Go, Python, Java, C#, Ruby, JS/Node, ...
  │
  ├── yes but only SharedArrayBuffer?
  │     └── use Atomics (~10ns)
  │         Browser JS, TypeScript, Dart/Flutter Web
  │
  ├── no mmap, but has stdin/stdout?
  │     └── use metropipe proxy (~100μs)
  │         Bash, AWK, Perl, PHP, Lua, Tcl, Julia, R, ...
  │
  └── no stdin/stdout (HTTP only)?
        └── use metropipe bridge (~1ms)
            curl, PowerShell, VBA, MATLAB, ...
```

## License

Apache 2.0 with runtime exception
