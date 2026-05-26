# metropipe

Export a function from one language and call it from any other. No compilation, no headers, no linker scripts, no `.so` files.

## How it's simpler than C linker files

With C, calling a function from another language means:

1. Write a `.h` header declaring the function
2. Write a `.c` implementation
3. Compile to a `.o` or `.so`
4. Write a linker script or use `dlopen`/`dlsym`
5. Figure out the ABI for each target language (calling conventions, struct padding, name mangling)
6. Recompile everything when the signature changes

With metropipe:

1. Pick a function in any language
2. `metropipe export classify services.py --target rust go c`
3. `python3 metropipe/classify/provider.py &`
4. Call `classify()` from Rust, Go, or C using the generated stub — no linking, no ABI

The provider just runs the function in a loop, reading requests from shared memory and writing responses. The generated stubs handle the serialization and the request/response cycle. Adding a new language is `--target js` — no recompilation, no new headers.

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

# The generated stubs have the same signature as the original:
# Python: def classify(image_path: str, top_k: int) -> (str, float):
# Rust:   pub fn classify(image_path: &str, top_k: i64) -> Result<(String, f64), String>
# Go:     func Classify(imagePath string, topK int) (string, float64, error)

# Run the provider — wraps your real classify() in a poll loop
python3 metropipe/classify/provider.py &

# Call from Rust (no linking, no headers, no .so)
let (label, conf) = classify("image.jpg", 3)?;
```

| Flag | Output pattern | Example |
|------|---------------|---------|
| `--namespace` (default) | `metropipe/classify/stub.rs` | One directory per function |
| `--flat` | `metropipe/classify.rs` | All in one directory |
| `--unify` | `metropipe.rs` | All functions merged per language |

`--target` repeats for multiple languages. Defaults to all 9 if omitted.

Source language is detected from the file extension: `.py`, `.rs`, `.go`, `.c`/`.h`, `.js`/`.mjs`, `.ts`, `.rb`, `.java`, `.cs`. Unknown extensions produce raw-bytes stubs.

Without a source file, generates a generic raw-bytes stub ready for custom serialization:

```bash
metropipe export Classifier
# produces metropipe/Classifier/stub.{rs,go,py,...} — fill in your own logic
```

### `connect` — Talk to a service from the terminal

```bash
metropipe connect WeatherApi                    # Interactive REPL
metropipe connect WeatherApi --send "London"    # One-shot
metropipe connect WeatherApi --listen           # Act as provider
metropipe connect WeatherApi --gen-stubs        # Generate stubs
```

The channel is created automatically on first use. No server needed.

### `proxy` — stdin/stdout bridge for languages without mmap

```bash
echo "New York" | metropipe proxy WeatherApi > response.bin
```

Any language that can read stdin and write stdout is a client: Bash, AWK, Perl, PHP, Lua, etc.

### `bind` — Generate stubs from a C header or library file

```bash
metropipe bind mylib.h
```

## How it works

```
┌──────────────────┐       ┌──────────────────────┐       ┌──────────────────┐
│  Rust (consumer) │────→  │  /dev/shm/metro_*   │ ←──── │  Python          │
│  classify()      │       │  32-byte header       │       │  classify()      │
│  generated stub  │       │  + JSON payload       │       │  provider loop   │
└──────────────────┘       │  atomic handshake     │       └──────────────────┘
                           └──────────────────────┘
                                  ↑
                          ┌───────┴────────┐
                          │  metropipe     │
                          │  proxy         │←── stdin (Bash, AWK, ...)
                          └────────────────┘
```

1. The first process to use a channel creates a file at `/dev/shm/metro_<name>` (or `/tmp/` on macOS, `./.metropipe/` as fallback). Set `$METROPIPE_DIR` to override.
2. Consumer writes a JSON payload, sets status word to `CONSUMER_REQ`.
3. Provider polls, sees the request, deserializes, calls the real function, serializes the result, sets status word to `PROVIDER_RES`.
4. Consumer reads the response, resets status word to `IDLE`.

No linker scripts. No ABI definitions. No `.so` files. The same file, same header, same handshake in every language.

## Language Support

| Language | Generated stub | How it connects |
|----------|---------------|-----------------|
| C | `stub.h` | mmap + atomic ops |
| Go | `stub.go` | syscall.Mmap |
| Python | `stub.py` | mmap |
| Java | `stub.java` | MappedByteBuffer |
| Rust | `stub.rs` | mmap + libc |
| C# | `stub.cs` | MemoryMappedFile |
| JavaScript | `stub.js` | SharedArrayBuffer |
| Ruby | `stub.rb` | IO.mmap |
| Bash | `stub.sh` | metropipe proxy |

## Protocol

All channels use a 32-byte header at the start of the shared memory file:

| Offset | Size | Field | Values |
|--------|------|-------|--------|
| 0 | 4 | STATUS_WORD | 0=idle, 1=request, 3=response, 4=error |
| 4 | 4 | CAS_LOCK | atomic mutex |
| 8 | 4 | PAYLOAD_SIZE | bytes written |
| 12 | 4 | MAX_CAPACITY | max payload |
| 16 | 4 | ERROR_CODE | error detail |
| 20 | 12 | (reserved) | padding |
| 32 | variable | PAYLOAD | JSON data |

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
├── clients/          # Reference implementations
├── Cargo.toml
└── docs/METROPOLITAN-SPEC.md
```

## Related

- [Brief Language](https://github.com/Randozart/brief-lang) — contract-verified builds
