# metropipe

Export a function from one language and call it from another. No linker scripts, no `.so` files, no ABI wrappers.

## How it works

You have a Python function `classify()` in `services.py`. You want Rust code and Go code to call it.

```bash
metropipe export classify services.py --target rust go
```

This reads the function signature from `services.py` and generates Rust and Go stubs with matching types. It also generates `metropipe/classify/provider.py`, which is a small script that imports your real `classify()` and runs it in a loop, listening for requests.

```bash
python3 metropipe/classify/provider.py &
```

Now any Rust or Go process on the same machine can call `classify()` as if it were a local function. The generated stub packs the arguments into JSON, sends them through a shared memory file, and waits for the response. The provider unpacks them, calls your real function, and sends back the result.

The shared memory file (`/dev/shm/metro_classify`) is created automatically on first use. Every language uses the same file, the same 32-byte header, and the same atomic handshake. There is no central server, no daemon, and no configuration.

Generated stubs use the same parameter names and types as the original function. If `classify` takes `image_path: str` and `top_k: int` and returns a `(str, float)`, the Rust stub generates `fn classify(image_path: &str, top_k: i64) -> Result<(String, f64), String>` and the Go stub generates `func Classify(imagePath string, topK int) (string, float64, error)`. The provider serializes everything to JSON automatically.

## Commands

### export

```bash
metropipe export <function> <source> --target <lang> <lang> ...
```

The `--target` flag accepts any of these languages: `c`, `go`, `python`, `java`, `rust`, `csharp`, `js`, `ruby`, `bash`. If omitted, stubs are generated for all of them.

Output can be structured three ways:

- `--namespace` (default) creates a directory per function: `metropipe/classify/stub.rs`
- `--flat` puts stubs in a single directory: `metropipe/classify.rs`
- `--unify` merges all exported functions into one file per language: `metropipe.rs`

Use `--out <dir>` to change where files are written.

If you run `export` without a source file, it generates generic raw-bytes stubs that you can fill in with your own serialization:

```bash
metropipe export Classifier
```

The source language is guessed from the file extension. Supported: `.py`, `.rs`, `.go`, `.c`, `.h`, `.js`, `.mjs`, `.ts`, `.rb`, `.java`, `.cs`. Anything else produces stubs that work with raw bytes.

### connect

```bash
metropipe connect WeatherApi
```

Opens the shared memory channel for a service and lets you send requests interactively. Each line you type is sent as a request, and the response is printed.

```
> New York
Response: sunny, 72°F
```

Other flags:

- `--send <data>` sends one request and prints the response, then exits.
- `--listen` puts you in provider mode: it waits for requests, prompts you for a response, sends it back.
- `--gen-stubs` generates client stub files for all supported languages.

The channel file is created automatically if it doesn't exist yet.

### proxy

For languages that can't call `mmap` (Bash, AWK, Perl, etc.), `proxy` wraps the shared memory handshake as stdin/stdout:

```bash
echo "New York" | metropipe proxy WeatherApi > response.bin
```

Reads lines from stdin, sends each as a request, writes each response to stdout.

### bind

```bash
metropipe bind mylib.h
```

Reads a C header file and generates stubs for all supported languages. Useful when you already have a library with its own types and want cross-language bindings without writing any glue code.

## Protocol

All channels use a 32-byte header at the start of a shared memory file:

- bytes 0-3: status word (0=idle, 1=request sent, 3=response ready, 4=error)
- bytes 4-7: atomic lock for concurrent access
- bytes 8-11: number of bytes written in the payload area
- bytes 12-15: maximum capacity of the payload area
- bytes 16-19: error code (set when status is 4)
- bytes 20-31: reserved
- bytes 32 onward: payload data

The handshake is straightforward: the consumer writes data into the payload area, sets the status to 1, and waits. The provider sees the status change, processes the data, writes a result into the payload area, and sets the status to 3. The consumer reads the result and resets the status to 0.

The file path is `/dev/shm/metro_<name>` on Linux, `/tmp/metro_<name>` on macOS, or `./.metropipe/metro_<name>` as a fallback. Set the environment variable `METROPIPE_DIR` to use a different directory.

## Language Support

| Language | Generated stub | How it connects to shared memory |
|----------|---------------|----------------------------------|
| C | `stub.h` | mmap + atomic operations |
| Go | `stub.go` | syscall.Mmap |
| Python | `stub.py` | mmap |
| Java | `stub.java` | MappedByteBuffer |
| Rust | `stub.rs` | mmap + libc |
| C# | `stub.cs` | MemoryMappedFile |
| JavaScript | `stub.js` | SharedArrayBuffer + Atomics |
| Ruby | `stub.rb` | IO.mmap |
| Bash | `stub.sh` | calls metropipe proxy |

## Project Structure

```
metropipe/
├── src/
│   ├── main.rs       # CLI commands
│   ├── export.rs     # Parses functions, generates stubs and providers
│   ├── channel.rs    # 32-byte header protocol helpers
│   ├── connect.rs    # Interactive REPL and one-shot RPC
│   ├── codegen.rs    # Per-language stub generation
│   └── proxy.rs      # stdin/stdout bridge
├── clients/          # Reference client implementations for all languages
├── Cargo.toml
└── docs/METROPOLITAN-SPEC.md
```

## Install

```bash
cargo install metropipe
```

Pre-built binaries are available on the [releases page](https://github.com/Randozart/metropipe/releases).
