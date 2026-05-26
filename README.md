# metropipe

Generates the code needed to call a function across processes on the same machine. If the function lives in another process, metropipe writes shared memory stubs. If it lives in the same process, metropipe can generate C function pointer stubs or linker-ready `.so` files — these are the same C ABI you'd use otherwise, just automated from your function signature.

## Where it fits

| Situation | metropipe gives you | Notes |
|-----------|---------------------|-------|
| Same machine, different processes, different languages | Shared memory stubs with zero-copy layout. The generated code handles mmap, handshake, and byte layout. No server, no serialization, no configuration. | Faster than gRPC on localhost. No protos, no daemon. Without metropipe you'd write this from scratch for every language combination. |
| Same machine, same process, two languages | Generated C stubs. `c-direct` produces a function pointer registry. `c-linker` produces a `.h` + `.c` + Makefile you compile to a `.so`. | These use the same C ABI you'd write by hand. metropipe generates them from your function signature so you don't type the boilerplate. |
| Different machines | Not the right tool. metropipe doesn't do networking. Use gRPC, Thrift, or HTTP. |
| Async queuing or persistence | Not the right tool. metropipe doesn't store or queue messages. Use Kafka, NATS, or Redis. |

## How it works

You have a Python function `classify()` in `services.py`. You want to call it from Rust or Go.

```bash
metropipe export classify services.py --target rust go
```

This reads the function signature from `services.py` and generates Rust and Go stubs with matching types. It also generates a provider script (`metropipe/classify/provider.py`) that imports your real `classify()` and runs it in a loop, waiting for requests over shared memory.

```bash
python3 metropipe/classify/provider.py &
```

Now any process on the same machine that imports the generated Rust or Go stub can call `classify()` and get a result back. The stub writes arguments into a shared memory buffer at computed byte offsets, sets a status flag, and waits for the provider to respond. The provider reads the buffer, calls your real function, and writes the result back.

For cross-process stubs, metropipe calculates a binary layout where each parameter has a fixed byte offset: 4 bytes for int, 4 for float, 256 for string, 1 for bool. The generated stubs and the provider both know this layout — no serialization or encoding between them.

The shared memory file is created on first use. There is no server, daemon, or configuration.

## Commands

### export

```bash
metropipe export <function> <source> --target <lang> <lang> ...
```

Supported targets: `c-direct`, `c-linker`, `c` (shared memory), `go`, `python`, `java`, `rust`, `csharp`, `js`, `ruby`, `bash`.

The three C targets:

- `c-direct` — generates a function pointer registry. Open `registry.h`, call `metropipe_get_registry()->classify(args)`. This is the same mechanism as a manual C plugin system, but metropipe writes the registration and lookup code.
- `c-linker` — generates `.h` + `.c` + `Makefile`. Run `make` to produce `libclassify.so`. This is the same process as writing a shared library by hand, but metropipe generates the interface files.
- `c` — shared memory stub for cross-process calls. No compilation, no linking, no server.

Output modes:

- `--namespace` (default): `metropipe/classify/stub.rs`
- `--flat`: `metropipe/classify.rs`
- `--unify`: `metropipe.rs` (appends on subsequent exports)

Use `--out <dir>` to change output directory.

Without a source file, generates raw-bytes stubs you fill in yourself.

Source language detection from extension: `.py`, `.rs`, `.go`, `.c`, `.h`, `.js`, `.mjs`, `.ts`, `.rb`, `.java`, `.cs`. Unknown extensions produce raw-bytes stubs.

### connect

```bash
metropipe connect WeatherApi
```

Interactive REPL over a shared memory channel. Lines are sent as request payloads, responses are printed. `--send <data>` for one-shot, `--listen` to act as provider, `--gen-stubs` to generate client library files.

### proxy

Wraps the shared memory handshake as stdin/stdout. For languages that cannot call mmap (Bash, AWK, Perl, etc.):

```bash
echo "payload" | metropipe proxy WeatherApi > response.bin
```

### bind

Reads a C header and generates stubs for all supported languages. Useful when you already have a library interface defined.

## Zero-copy layout

Since `export` knows the function signature at generation time, it calculates fixed byte offsets for each parameter. Both the stub and the provider use the same layout:

| Type | Buffer size |
|------|-------------|
| int32, float | 4 bytes |
| int64, double | 8 bytes |
| String | 256 bytes |
| bool | 1 byte |
| Data, bytes | 4096 bytes |

No encoding, no decoding, no allocation during the call.

## Protocol

Shared memory channels use a 32-byte header at the file start:

- bytes 0-3: status word (0=idle, 1=request, 3=response, 4=error)
- bytes 4-7: atomic lock
- bytes 8-11: payload size
- bytes 12-15: maximum capacity
- bytes 16-19: error code
- bytes 20-31: reserved
- bytes 32 onward: payload (zero-copy layout)

Path resolution: `/dev/shm/metro_<name>` (Linux), `/tmp/metro_<name>` (macOS), `./.metropipe/metro_<name>` (fallback). Override with `METROPIPE_DIR`.

## Language Support

| Target | Generated file(s) | Mechanism |
|--------|-------------------|-----------|
| C (shm) | `stub.h` | mmap + atomics |
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

## Install

```bash
cargo install metropipe
```

Pre-built binaries on the [releases page](https://github.com/Randozart/metropipe/releases).
