# metropipe

**The Universal Language Binder** — Zero-copy shared memory IPC between any two languages. No C-ABI. No serialization. No wrappers.

## What

`metropipe` lets a Python script, a Node.js server, a C program, and a Brief reactor all exchange data through the same block of shared memory — at ~10ns latency, using atomic CAS for coordination. No function calls. No serialization. No `.so` files.

The protocol is a 32-byte header + payload plane in `/dev/shm/metro_{service}`. Any language with `mmap` can participate.

## Quick Start

```bash
# 1. Start the daemon
./metropipe

# 2. Connect from Python
from metropipe import MetroClient
with MetroClient("WeatherApi") as client:
    result = client.send(b"New York", timeout_ms=5000)

# 3. Or use the universal CLI
brief metropipe connect WeatherApi
> city = "New York"
Response: temperature=72.5, humidity=0.45, condition="Sunny"
```

## Protocol

All channels use a 32-byte control header + variable-size payload:

| Offset | Field | Values |
|--------|-------|--------|
| 0x00 | STATUS_WORD | 0=IDLE, 1=CONSUMER_REQ, 2=PROVIDER_ACK, 3=PROVIDER_RES, 4=ERROR |
| 0x04 | CAS_LOCK | Atomic mutex |
| 0x08 | PAYLOAD_SIZE | Bytes written |
| 0x0C | MAX_CAPACITY | Max payload size |
| 0x10 | ERROR_CODE | Error details |
| 0x14 | RESERVED | Padding |
| 0x20 | PAYLOAD | Data |

## Clients

| Language | File |
|----------|------|
| C/C++ | `clients/c/metropipe.h` |
| Python | `clients/python/metropipe.py` |
| JavaScript/Node | `clients/javascript/metropipe.js` |
| Brief | `lib/std/metro_bridge.bv` (via compiler) |

## Project Structure

```
metropipe/
├── metropipe                  # Compiled binary
├── src/metropipe.bv           # Daemon (Brief source)
├── clients/
│   ├── c/metropipe.h          # C header
│   ├── python/metropipe.py    # Python client
│   └── javascript/metropipe.js# Node.js client
├── docs/METROPOLITAN-SPEC.md  # Full protocol spec
├── examples/services.dbv      # Example service IDL
├── PLAN.md                    # Development roadmap
└── Makefile
```

## See Also

- [METROPOLITAN-SPEC.md](docs/METROPOLITAN-SPEC.md) — Full protocol specification
- [PLAN.md](PLAN.md) — Development roadmap
- [../brief-compiler/](../brief-compiler/) — Brief compiler with `brief metropipe connect`
