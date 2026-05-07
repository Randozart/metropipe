# Metro Daemon (metrod)

**The Metropolitan FFI Broker** — A universal, zero-copy, language-agnostic IPC protocol.

Metro Daemon manages shared-memory service registration and lookup, enabling any programming language to communicate with any other at the speed of RAM — no serialization, no network stack, no C-ABI wrappers. This is accomplished by using the Brief programming language.

## Architecture

```
┌──────────────┐     .dbv IDL     ┌─────────────┐     memory-spec.json     ┌──────────────┐
│   Python     │ ───────────────► │  metrod     │ ◄────────────────────── │   Node.js    │
│   Client     │                  │  (Brief)    │                         │   Client     │
└──────────────┘                  └──────┬──────┘                         └──────────────┘
                                         │
                                  /dev/shm/metro_*
                                  (shared memory)
                                         │
┌──────────────┐                  ┌──────┴──────┐                         ┌──────────────┐
│   Rust       │ ◄──────────────► │  Metro FFI  │ ◄────────────────────── │   C/C++      │
│   Client     │                  │  Protocol   │                         │   Client     │
└──────────────┘                  └─────────────┘                         └──────────────┘
```

## The Metropolitan Stack

| Layer | Component | Description |
|-------|-----------|-------------|
| **Protocol** | Metropolitan FFI | 32-byte header, atomic CAS, status words |
| **IDL** | `.dbv` files | Service schema definitions (d-brief format) |
| **Broker** | `metrod` | Shared memory allocator and registry |
| **Clients** | Python, JS, C | Language-specific client libraries |

## Quick Start

### 1. Build the Brief Compiler

```bash
cd ../brief-compiler
cargo build --release
```

### 2. Compile the Metro Daemon

```bash
cd ../brief-compiler
./target/release/brief-compiler build ../metrod/src/metrod.bv
cp metrod ../metrod/metrod
```

Or simply:

```bash
make build
```

The compiled binary lands at `./metrod` (3.9 MB native executable).

### 3. Run the Daemon

```bash
./metrod
```

The daemon starts as a reactive state machine, initializing its service registry
and waiting for commands via shared memory variables.

### 4. Define a Service (IDL)

Create a `weather.dbv` file:

```brief
SERVICE WeatherApi {
    INPUT city: String;
    OUTPUT temperature: Float;
    OUTPUT humidity: Float;
    OUTPUT condition: String;
}
```

### 5. Use from Python

```python
from metro import MetroClient

with MetroClient("WeatherApi") as client:
    # Pack request: city name as bytes
    request = b"New York\x00" * 32  # 256 bytes
    response = client.send(request, timeout_ms=5000)
    print(f"Got {len(response)} bytes response")
```

### 6. Use from Node.js

```javascript
const { MetroClient } = require('./clients/javascript/metro.js');

const client = new MetroClient('WeatherApi');
const request = Buffer.alloc(256, 0);
request.write('New York');
const response = await client.request(request);
console.log(`Got ${response.length} bytes response`);
client.close();
```

### 7. Use from C

```c
#include "clients/c/metro.h"

int main() {
    MetroChannel ch;
    metro_channel_open(&ch, "/dev/shm/metro_WeatherApi");

    uint8_t request[256] = {0};
    memcpy(request, "New York", 8);
    metro_channel_send(&ch, request, 256);

    uint8_t response[1024];
    int len = metro_channel_recv(&ch, response, sizeof(response), 5000);

    metro_channel_close(&ch);
    return 0;
}
```

## Protocol Spec

See [docs/METROPOLITAN-SPEC.md](docs/METROPOLITAN-SPEC.md) for the full technical specification, including:
- 32-byte header layout
- Status word lifecycle
- Handshake protocol
- Reference implementations
- Hardware (FPGA) synthesis

## Project Structure

```
metrod/
├── metrod                       # Compiled binary (native executable)
├── src/
│   └── metrod.bv                # Metro Daemon (Brief source)
├── examples/
│   └── services.dbv             # Example service IDL definitions
├── clients/
│   ├── python/metro.py          # Python client library
│   ├── javascript/metro.js      # Node.js client library
│   └── c/metro.h                # C client header
├── docs/
│   └── METROPOLITAN-SPEC.md     # Protocol specification
├── Makefile
└── README.md
```

## Why Metropolitan?

| Feature | Metropolitan | gRPC | Redis |
|---------|-------------|------|-------|
| Latency | ~10ns | ~1-10ms | ~1ms |
| Serialization | Zero-copy | Protobuf | RESP |
| Cross-language | Any with mmap | Language-specific | Any with TCP |
| FPGA support | Yes (MMIO) | No | No |
| Blocking | Non-blocking | Blocking | Blocking |

## License

Apache License 2.0 with runtime exception

## Author

Randy Smits-Schreuder Goedheijt
