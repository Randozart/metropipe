# metropipe — Universal Language Binder

**Make any two languages talk at RAM speed. No C-ABI. No serialization. No wrappers.**

## The Goal

`metropipe` is a universal, zero-copy, language-agnostic shared-memory IPC bus. Any language with `mmap` (or `SharedArrayBuffer`, or MMIO) can talk to any other — at ~10ns latency, no serialization overhead, no function call ABI.

The binary is a simple reactive daemon. `brief metropipe connect` is a CLI that makes it trivial to use.

```bash
# Start a service
$ metropipe

# Connect from the command line (auto-detects schema)
$ brief metropipe connect WeatherApi
Connected to /dev/shm/metro_WeatherApi
> city = "New York"
Response: temperature=72.5, humidity=0.45, condition="Sunny"

# Or generate client stubs for embeddng
$ brief metropipe connect WeatherApi --gen-stubs
  → metropipe_WeatherApi.h
  → metropipe_WeatherApi.py
  → metropipe_WeatherApi.js
```

## Architecture

```
┌──────────┐     ┌──────────────────┐     ┌──────────┐
│  Python  │────→│   shared memory  │←────│  Node.js │
│  Client  │     │  /dev/shm/metro_*│     │  Client  │
└──────────┘     │  32-byte header  │     └──────────┘
                 │  + payload plane │
┌──────────┐     │  atomic CAS lock │     ┌──────────┐
│  C/C++   │────→│  status word     │←────│  Brief   │
│  Client  │     └──────────────────┘     │  Runtime │
└──────────┘                               └──────────┘
```

### The Protocol: 32-byte Header

All fields are 32-bit LE u32:

| Offset | Field | Description |
|--------|-------|-------------|
| 0x00 | STATUS_WORD | 0=IDLE, 1=CONSUMER_REQ, 2=PROVIDER_ACK, 3=PROVIDER_RES, 4=ERROR |
| 0x04 | CAS_LOCK | Atomic compare-and-swap mutex |
| 0x08 | PAYLOAD_SIZE | Bytes written in payload |
| 0x0C | MAX_CAPACITY | Maximum payload size |
| 0x10 | ERROR_CODE | Error metadata if STATUS=4 |
| 0x14 | RESERVED | 12 bytes padding |
| 0x20 | PAYLOAD | Raw byte data |

Handshake: `IDLE → CONSUMER_REQ → PROVIDER_ACK → PROVIDER_RES → IDLE`

## Phases

### Phase R: Rename (2 hours)

**Files to rename in metropipe repo:**
- `src/metrod.bv` → `src/metropipe.bv`
- `clients/python/metro.py` → `clients/python/metropipe.py`
- `clients/python/__init__.py` (create, exports MetroClient/MetroBroker as aliases)
- `clients/javascript/metro.js` → `clients/javascript/metropipe.js`
- `clients/c/metro.h` → `clients/c/metropipe.h`
- `Makefile` — update binary name, target names
- `README.md` — full rewrite as metropipe

**References in brief-compiler:**
- `src/ffi/metro_cli.rs` — rename internal strings, command name "metrod"→"metropipe"
- `src/main.rs` — update help text for `brief metropipe connect`
- `lib/std/metro_bridge.bv` — update doc comments
- `src/ffi/metropolitan.rs` — update codegen header guards/comments

---

### Phase G: Protocol Alignment (3 days)

**G1 — `generate_metropipe_c_header()` in `metropolitan.rs`**

Current `generate_c_header()` emits the 3-region protocol (separate req/resp/sync regions with 64-bit status words). Add a second codegen path that emits metropipe's single-region 32-byte header protocol:

```rust
pub fn generate_metropipe_c_header(&self, channel_id: &str) -> Result<String, String>
pub fn generate_metropipe_python_module(&self, channel_id: &str) -> Result<String, String>
pub fn generate_metropipe_js_module(&self, channel_id: &str) -> Result<String, String>
```

The output must exactly match the `clients/c/metropipe.h` layout (same offsets, same status word values, same atomic CAS handshake).

**G2 — Wire into `brief metropipe connect` CLI**

- `--lang c` → use `generate_metropipe_c_header()`
- `--lang python` → use `generate_metropipe_python_module()`
- `--lang js` → use `generate_metropipe_js_module()`

**G3 — Update `metro_bridge.bv`**

The frgn declarations need to work with the 32-byte header offsets. The bridge currently uses native Rust impls (shm_open, mmap, atomic CAS) — these are OS-level calls that work regardless of the header layout. Update `metropolitan_rpc()` to write at the correct offsets (0x20 for payload, 0x00 for status, etc.)

---

### Phase H: `brief bind` → `.dbv` Services (2 days)

When `brief bind mylib.h` runs, additionally emit:

- **`service.dbv`**: Metropolitan IDL definition (SERVICE/INPUT/OUTPUT)
- **`metropipe_stub.h`**: C client header using the 32-byte protocol
- **`metropipe_stub.py`**: Python client using the 32-byte protocol
- **`metropipe_stub.js`**: JS client using the 32-byte protocol
- **`memory-spec.json`**: Field offset/size layout for schema-aware clients

This makes `brief bind` produce a fully working cross-language service in one command:

```bash
$ brief bind mylib.h --gen-stubs
  Analyzed 12 functions in mylib.h
  Generated:
    lib/ffi/generated/mylib/bridge.bv      # Brief frgn declarations
    lib/ffi/generated/mylib/service.dbv    # Metropolitan IDL
    lib/ffi/generated/mylib/metropipe_stub.h  # C client
    lib/ffi/generated/mylib/metropipe_stub.py # Python client
    lib/ffi/generated/mylib/metropipe_stub.js  # JS client
    lib/ffi/generated/mylib/memory-spec.json   # Schema layout

Now connect any language: metropipe connect mylib
```

---

### Phase I: `brief metropipe connect` Full RPC (2 days)

The CLI currently exists (`src/ffi/metro_cli.rs`) but uses the old 3-region protocol. Rewrite to:

1. **True 32-byte protocol**: Opens `/dev/shm/metro_{name}`, implements the spec handshake
2. **Schema-aware REPL**: If `memory-spec.json` exists, shows field names and types, accepts structured input
3. **`--listen` mode**: Act as a provider — receives requests, prompts for response
4. **`--gen-stubs` mode**: Generate embeddable client library files
5. **Raw mode**: Work without a schema (just send/receive raw bytes)

```bash
$ brief metropipe connect WeatherApi         # Schema-aware REPL
$ brief metropipe connect WeatherApi --raw   # Raw byte mode
$ brief metropipe connect WeatherApi --listen  # Act as provider
$ brief metropipe connect WeatherApi --gen-stubs  # Generate stubs
```

---

### Phase J: Daemon Expansion (1 week)

The current daemon (`src/metropipe.bv`) is 110 lines of reactive state machines that track `service_count` in `let` variables. It does NOT actually allocate shared memory.

1. **Actual shm_alloc**: Use the metropolitan FFI bridge (`__shm_open`, `__ftruncate`, `__mmap_anonymous`) to create real `/dev/shm/metro_{name}` files when a service is registered
2. **Real registry**: A `Map<String, ServiceDef>` storing service names → schemas → memory addresses
3. **Hot-reload**: Watch a directory for `.dbv` files; auto-register services when files appear
4. **Health check**: Expose status via a unix socket or a well-known shared memory path (`/dev/shm/metro__health`)

---

### Phase K: Stub Generators in Brief (3 days)

Write a Brief program that reads a `.dbv` service definition and emits C/Python/JS client stubs using string operations and the metropolitan FFI bridge for file I/O.

This proves the full FFI cycle:

```
Brief daemon (metropipe.bv)       — reactive state machine managing shared memory
Brief bridge (metro_bridge.bv)    — frgn declarations for OS primitives
Brief stub generator (NEW)        — reads .dbv, emits C/Python/JS stubs
Generated stubs talk to daemon    — via shared memory, same protocol
All via brief metropipe connect    — the CLI that ties it together
```

---

## Files Changed Per Phase

| Phase | Files |
|-------|-------|
| **R** | `metropipe/` folder rename, `src/metro_cli.rs`, `src/main.rs`, `lib/std/metro_bridge.bv`, `src/ffi/metropolitan.rs` |
| **G** | `src/ffi/metropolitan.rs`, `src/ffi/metro_cli.rs`, `lib/std/metro_bridge.bv` |
| **H** | `src/wrapper/generator.rs`, `src/wrapper/mod.rs`, `src/ffi/metropolitan.rs` |
| **I** | `src/ffi/metro_cli.rs` → `src/ffi/metropipe_cli.rs` |
| **J** | `metropipe/src/metropipe.bv` |
| **K** | `lib/std/metropipe_gen.bv` (NEW) |
