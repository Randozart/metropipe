# metropipe

Export a function from one language and call it from another.

## How it works

You have a Python function `classify()` in `services.py`. You want Rust code and Go code to call it.

```bash
metropipe export classify services.py --target rust go
```

This reads the function signature from `services.py` and generates Rust and Go stubs with matching types. It also generates `metropipe/classify/provider.py`, which imports your real `classify()` and runs it in a loop, listening for requests.

```bash
python3 metropipe/classify/provider.py &
```

Now any Rust or Go process on the same machine can call `classify()` as if it were a local function. The generated stub writes arguments directly into a shared memory buffer at known byte offsets, sets a status flag, and waits for the provider to write back. No serialization, no JSON, just raw bytes at deterministic positions.

The shared memory file (`/dev/shm/metro_classify`) is created on first use. No server, no daemon, no configuration.

## Commands

### export

```bash
metropipe export <function> <source> --target <lang> <lang> ...
```

The `--target` flag accepts any of these: `c` (shared memory), `c-direct` (function pointer registry), `c-linker` (compiled `.so`), `go`, `python`, `java`, `rust`, `csharp`, `js`, `ruby`, `bash`. If omitted, stubs are generated for all of them.

The three C targets serve different use cases:

| Target | Latency | Setup | Best for |
|--------|---------|-------|----------|
| `c-direct` | ~2ns | Provider calls `metropipe_register()` at startup. Consumer gets a function pointer from `metropipe_get_registry()`. No compilation, no shared memory. | Same-process embedding where both sides are compiled together. |
| `c-linker` | ~1ns | Generates `.h` + `.c` + `Makefile`. Run `make` to produce a `.so`. Link with `-lclassify`. | Same-process ABI boundary. Standard C FFI. |
| `c` | ~10ns | Shared memory stub. No compilation, no linking. Provider runs independently. | Cross-process communication. Hot-reloadable. |

Output can be structured three ways:

- `--namespace` (default) creates a directory per function: `metropipe/classify/stub.rs`
- `--flat` puts stubs in a single directory: `metropipe/classify.rs`
- `--unify` merges all exported functions into one file per language: `metropipe.rs`

Use `--out <dir>` to change where files are written.

If you run `export` without a source file, it generates generic stubs that you can fill in with your own logic:

```bash
metropipe export Classifier
```

The source language is guessed from the file extension. Supported: `.py`, `.rs`, `.go`, `.c`, `.h`, `.js`, `.mjs`, `.ts`, `.rb`, `.java`, `.cs`.

### connect

```bash
metropipe connect WeatherApi
```

Opens the shared memory channel for a service and lets you send requests interactively. Each line is sent as a request, the response is printed.

- `--send <data>` sends one request and exits.
- `--listen` puts you in provider mode.
- `--gen-stubs` generates client stub files for all supported languages.

### proxy

For languages that can't call mmap (Bash, AWK, Perl, etc.), `proxy` wraps the shared memory handshake as stdin/stdout:

```bash
echo "New York" | metropipe proxy WeatherApi > response.bin
```

### bind

```bash
metropipe bind mylib.h
```

Reads a C header file and generates stubs for all supported languages.

## Zero-copy layout

The `export` command knows the function signature — parameter names, types, and their order. Instead of serializing to JSON, it calculates fixed byte positions for each parameter:

| Type | Size in buffer |
|------|---------------|
| int32 / float | 4 bytes |
| int64 / double | 8 bytes |
| String | 256 bytes |
| bool | 1 byte |
| Data / bytes | 4096 bytes |

Both the generated stub and the generated provider know this layout. Writing and reading happens at known offsets — no encoding, no decoding, no allocation.

## Protocol

Shared memory channels use a 32-byte header at the start of the file:

- bytes 0-3: status word (0=idle, 1=request, 3=response, 4=error)
- bytes 4-7: atomic lock
- bytes 8-11: payload size
- bytes 12-15: maximum capacity
- bytes 16-19: error code
- bytes 20-31: reserved
- bytes 32 onward: payload (zero-copy layout)

The consumer writes data at the correct byte offsets, sets status to 1. The provider sees the change, processes, writes the result at the correct offsets, sets status to 3. The consumer reads and resets status to 0.

The file path is `/dev/shm/metro_<name>` on Linux, `/tmp/metro_<name>` on macOS, or `./.metropipe/metro_<name>` as fallback. Set `METROPIPE_DIR` to use a different directory.

## Language Support

| Language | Generated stub | How it connects |
|----------|---------------|-----------------|
| C (shm) | `stub.h` | mmap + atomic operations |
| C (direct) | `registry.h` | function pointer table |
| C (linker) | `.h` + `.c` + `Makefile` | compiled `.so` |
| Go | `stub.go` | syscall.Mmap |
| Python | `stub.py` | mmap |
| Java | `stub.java` | MappedByteBuffer |
| Rust | `stub.rs` | mmap + libc |
| C# | `stub.cs` | MemoryMappedFile |
| JavaScript | `stub.js` | SharedArrayBuffer |
| Ruby | `stub.rb` | IO.mmap |
| Bash | `stub.sh` | calls metropipe proxy |

## Project Structure

```
metropipe/
├── src/
│   ├── main.rs       # CLI: export, connect, bind, proxy
│   ├── export.rs     # Function parsing + stub generation
│   ├── channel.rs    # 32-byte header protocol
│   ├── connect.rs    # Interactive REPL
│   ├── codegen.rs    # Legacy stub generator
│   └── proxy.rs      # stdin/stdout bridge
├── Cargo.toml
└── docs/METROPOLITAN-SPEC.md
```

## Install

```bash
cargo install metropipe
```

Pre-built binaries on the [releases page](https://github.com/Randozart/metropipe/releases).
