# Metropolitan Shared Memory FFI Protocol

**Version:** 1.0
**Author:** Randy Smits-Schreuder Goedheijt (Brief Language Architect)
**Status:** Draft Specification

## 1. Abstract

Traditional Foreign Function Interfaces (FFI) rely on the C-ABI, which mandates thread-blocking function calls, stack manipulation, and high context-switching overhead. For reactive, formally verified state machines (like those compiled by Brief) or spatial computing environments (FPGAs), blocking I/O is an anti-pattern.

The **Metropolitan Protocol** replaces the C-ABI with an asynchronous, shared-memory negotiation model. It treats cross-language communication as atomic memory mutations. This allows any host environment—whether a Python script, a Node.js web server, or a physical FPGA hardware pin—to safely exchange data with a Brief reactor loop at the speed of RAM, without ever blocking the execution thread.

## 2. Architecture

### 2.1 Components

The Metropolitan stack consists of four layers:

1. **Metropolitan FFI Protocol** — The underlying physics: 32-byte header, atomic CAS locks, status words.
2. **Metropolitan IDL** (via `.dbv` files) — The schema language that describes the shape of data exchanged between services.
3. **Metro Daemon (`metrod`)** — The traffic controller that reads IDL files, allocates shared memory, and generates memory specs.
4. **Metro Bindgen** — The compiler tool that generates native client wrappers (Python, JS, C) from memory specs.

### 2.2 Design Philosophy

> *"There are no function calls. There is only a shared memory buffer and a handshake."*

Metropolitan bypasses the traditional C-ABI entirely. Instead of writing wrappers, compiling `.so` files, and using `libffi` to push arguments onto the CPU stack, languages simply map the same block of memory into their address space and coordinate via atomic status words.

## 3. Memory Layout

Every Metropolitan channel allocates a contiguous block of shared memory (via `/dev/shm`, `mmap`, `SharedArrayBuffer`, or MMIO).

The memory region is strictly divided into a **32-byte Control Header** and a **Data Payload Plane**. All header fields are 32-bit (4-byte) Little-Endian unsigned integers (`u32`).

### 3.1 The 32-Byte Control Header

| Offset | Size (Bytes) | Field Name | Description |
| :--- | :--- | :--- | :--- |
| `0x00` | 4 | `STATUS_WORD` | Atomic u32 tracking the current state transition. |
| `0x04` | 4 | `CAS_LOCK` | Atomic u32 for Compare-And-Swap mutual exclusion. |
| `0x08` | 4 | `PAYLOAD_SIZE` | Size (in bytes) of the active data in the payload. |
| `0x0C` | 4 | `MAX_CAPACITY` | Maximum allocated size for the payload plane. |
| `0x10` | 4 | `ERROR_CODE` | Contains error metadata if `STATUS_WORD` == 5. |
| `0x14` | 12 | `RESERVED` | Padding for 32-byte alignment / future extensions. |
| `0x20` | `MAX_CAPACITY` | `PAYLOAD` | The raw byte data representing the structs/variables. |

### 3.2 Status Word Enums

The `STATUS_WORD` dictates the lifecycle of the transaction:

| Value | Name | Description |
| :--- | :--- | :--- |
| `0` | `IDLE` | The channel is free. No party is using the buffer. |
| `1` | `CONSUMER_REQ` | The Consumer has written data and awaits Provider processing. |
| `2` | `PROVIDER_ACK` | The Provider has locked the buffer and is processing. |
| `3` | `PROVIDER_RES` | The Provider has finished; result data is in the payload. |
| `4` | `ERROR` | A timeout or boundary violation occurred. Check `ERROR_CODE` at `0x10`. |

## 4. The Handshake Protocol

### 4.1 Consumer-to-Provider Request

To send data to a Provider, a Consumer executes:

1. **Wait for IDLE:** Read `STATUS_WORD` at offset `0x00`. If not `0`, yield or sleep.
2. **Acquire Lock:** Perform an Atomic Compare-and-Swap on `CAS_LOCK` at `0x04` (Expected `0`, New `1`).
3. **Write Payload:** Pack data into raw bytes starting at offset `0x20`.
4. **Update Size:** Write the total bytes written to `PAYLOAD_SIZE` at `0x08`.
5. **Signal:** Write `1` (`CONSUMER_REQ`) to `STATUS_WORD` at `0x00`.
6. **Release Lock:** Write `0` to `CAS_LOCK` at `0x04`.

### 4.2 Provider Response

The Provider (e.g., a Brief reactor loop) detects `CONSUMER_REQ` and executes:

1. **Acquire Lock:** Atomic CAS on `CAS_LOCK` (Expected `0`, New `1`).
2. **Read Payload:** Read data from offset `0x20`, size from `PAYLOAD_SIZE`.
3. **Process:** Execute the state transition / function.
4. **Write Result:** Write output to the payload region.
5. **Update Size:** Write result size to `PAYLOAD_SIZE`.
6. **Signal:** Write `3` (`PROVIDER_RES`) to `STATUS_WORD`.
7. **Release Lock:** Write `0` to `CAS_LOCK`.

### 4.3 Consumer Read Response

1. **Wait for PROVIDER_RES:** Poll `STATUS_WORD` until it equals `3`.
2. **Read Result:** Read data from offset `0x20`, size from `PAYLOAD_SIZE`.
3. **Reset:** Write `0` (`IDLE`) to `STATUS_WORD` to free the channel.

## 5. Metropolitan IDL (`.dbv`)

The Metropolitan IDL uses the d-brief format extended with `SERVICE` declarations. This defines the memory layout that `metrod` allocates and that clients use to interpret the payload.

### 5.1 Syntax

```brief
SERVICE ServiceName {
    INPUT field_name: Type;
    OUTPUT field_name: Type;
}
```

### 5.2 Supported Types

| Brief Type | C Type | Size (bytes) | Notes |
| :--- | :--- | :--- | :--- |
| `Bool` | `uint8_t` | 1 | |
| `Int` | `int64_t` | 8 | Signed 64-bit |
| `UInt[8]` | `uint8_t` | 1 | Unsigned 8-bit |
| `UInt[16]` | `uint16_t` | 2 | Unsigned 16-bit |
| `UInt[32]` | `uint32_t` | 4 | Unsigned 32-bit |
| `UInt[64]` | `uint64_t` | 8 | Unsigned 64-bit |
| `Float` | `double` | 8 | IEEE 754 64-bit |
| `String` | `char[256]` | 256 | Fixed-size buffer |
| `Vector[T, N]` | `T[N]` | `sizeof(T) * N` | Fixed-size array |

### 5.3 Example

```brief
SERVICE WeatherApi {
    INPUT city: String;
    OUTPUT temperature: Float;
    OUTPUT humidity: Float;
    OUTPUT condition: String;
}
```

### 5.4 Generated Memory Spec

When `metrod` processes this IDL, it outputs a `memory-spec.json`:

```json
{
  "channel": {
    "address": "/dev/shm/metro_WeatherApi",
    "header_offset": 0,
    "payload_offset": 32,
    "capacity": 1024,
    "input_fields": 1,
    "output_fields": 3,
    "layout": {
      "city": { "offset": 0, "size": 256 },
      "temperature": { "offset": 256, "size": 8 },
      "humidity": { "offset": 264, "size": 8 },
      "condition": { "offset": 272, "size": 256 }
    }
  }
}
```

## 6. Reference Implementations

### 6.1 Python Client (`mmap`)

```python
import mmap
import struct
import time

class MetropolitanChannel:
    STATUS_IDLE = 0
    STATUS_CONSUMER_REQ = 1
    STATUS_PROVIDER_RES = 3

    def __init__(self, shm_path: str):
        with open(shm_path, "r+b") as f:
            self.shm = mmap.mmap(f.fileno(), 0)

    def wait_idle(self):
        while struct.unpack_from("<I", self.shm, 0)[0] != self.STATUS_IDLE:
            time.sleep(0.001)

    def send_request(self, payload_bytes: bytes):
        self.wait_idle()
        self.shm[32:32 + len(payload_bytes)] = payload_bytes
        struct.pack_into("<I", self.shm, 8, len(payload_bytes))
        struct.pack_into("<I", self.shm, 0, self.STATUS_CONSUMER_REQ)

    def wait_response(self, timeout_ms: int = 5000) -> bytes:
        start = time.time()
        while time.time() - start < timeout_ms / 1000:
            status = struct.unpack_from("<I", self.shm, 0)[0]
            if status == self.STATUS_PROVIDER_RES:
                size = struct.unpack_from("<I", self.shm, 8)[0]
                result = bytes(self.shm[32:32 + size])
                struct.pack_into("<I", self.shm, 0, self.STATUS_IDLE)
                return result
            time.sleep(0.001)
        raise TimeoutError("Provider did not respond")
```

### 6.2 JavaScript / WebAssembly Client (`SharedArrayBuffer`)

```javascript
class MetropolitanWasmChannel {
    static STATUS_IDLE = 0;
    static STATUS_CONSUMER_REQ = 1;
    static STATUS_PROVIDER_RES = 3;

    constructor(sharedBuffer) {
        this.header = new Int32Array(sharedBuffer, 0, 8);
        this.payload = new Uint8Array(sharedBuffer, 32);
    }

    async waitIdle() {
        while (Atomics.load(this.header, 0) !== MetropolitanWasmChannel.STATUS_IDLE) {
            await new Promise(r => setTimeout(r, 1));
        }
    }

    async sendRequest(dataArray) {
        await this.waitIdle();
        this.payload.set(dataArray);
        this.header[2] = dataArray.length; // PAYLOAD_SIZE at offset 0x08 (index 2)
        Atomics.store(this.header, 0, MetropolitanWasmChannel.STATUS_CONSUMER_REQ);
        Atomics.notify(this.header, 0, 1);
    }

    async waitResponse(timeoutMs = 5000) {
        const start = Date.now();
        while (Date.now() - start < timeoutMs) {
            if (Atomics.load(this.header, 0) === MetropolitanWasmChannel.STATUS_PROVIDER_RES) {
                const size = this.header[2];
                const result = this.payload.slice(0, size);
                Atomics.store(this.header, 0, MetropolitanWasmChannel.STATUS_IDLE);
                return result;
            }
            await new Promise(r => setTimeout(r, 1));
        }
        throw new Error("Provider did not respond");
    }
}
```

### 6.3 C Client Header

```c
#include <stdint.h>
#include <stdatomic.h>

#define METRO_HEADER_SIZE   32
#define METRO_STATUS_OFFSET 0
#define METRO_SIZE_OFFSET   8
#define METRO_PAYLOAD_OFFSET 32

#define METRO_STATUS_IDLE        0
#define METRO_STATUS_CONSUMER_REQ 1
#define METRO_STATUS_PROVIDER_RES 3

typedef volatile struct {
    _Atomic uint32_t status_word;
    _Atomic uint32_t cas_lock;
    _Atomic uint32_t payload_size;
    _Atomic uint32_t max_capacity;
    _Atomic uint32_t error_code;
    uint8_t reserved[12];
    uint8_t payload[];
} MetroChannel;

static inline void metro_wait_idle(MetroChannel *ch) {
    while (atomic_load(&ch->status_word) != METRO_STATUS_IDLE) {
        // Spin wait or yield
    }
}

static inline void metro_send(MetroChannel *ch, const uint8_t *data, uint32_t len) {
    metro_wait_idle(ch);
    for (uint32_t i = 0; i < len; i++) {
        ch->payload[i] = data[i];
    }
    atomic_store(&ch->payload_size, len);
    atomic_store(&ch->status_word, METRO_STATUS_CONSUMER_REQ);
}

static inline int metro_recv(MetroChannel *ch, uint8_t *out, uint32_t max_len, int timeout_ms) {
    // Poll with timeout
    uint32_t elapsed = 0;
    while (elapsed < timeout_ms) {
        if (atomic_load(&ch->status_word) == METRO_STATUS_PROVIDER_RES) {
            uint32_t size = atomic_load(&ch->payload_size);
            if (size > max_len) size = max_len;
            for (uint32_t i = 0; i < size; i++) {
                out[i] = ch->payload[i];
            }
            atomic_store(&ch->status_word, METRO_STATUS_IDLE);
            return size;
        }
        elapsed += 1; // usleep(1000)
    }
    return -1; // Timeout
}
```

## 7. Hardware Synthesis (FPGA / Verilog)

When Brief is compiled to FPGA targets, the Metropolitan region is not POSIX `/dev/shm`. It is synthesized as an **AXI4-Lite Memory Mapped BRAM**.

The `STATUS_WORD` is physically wired to a hardware interrupt line. When an external peripheral writes `1` to the `STATUS_WORD` register, it physically pulls the trigger pin HIGH, sparking the Brief state machine at the speed of electricity.

The exact same Python/C code written above works whether Brief is running as a software process on Linux, or synthesized into SystemVerilog running on an FPGA. The client code just writes to memory—it doesn't care if that memory address is `/dev/shm/metro_Service` or an MMIO PCIe address.

## 8. Security Considerations

1. **Memory Isolation:** Each service gets its own named shared memory region. Clients can only access regions they have been given the path to.
2. **Atomic Operations:** All status word mutations use atomic CAS to prevent race conditions between multiple consumers.
3. **Bounds Checking:** The `PAYLOAD_SIZE` field is validated against `MAX_CAPACITY` before any read/write operation.
4. **Timeout Enforcement:** Consumers should always implement timeout logic to prevent indefinite blocking if a Provider crashes.

## 9. Comparison to Existing Standards

| Feature | Metropolitan | gRPC + Protobuf | Apache Arrow | Redis |
| :--- | :--- | :--- | :--- | :--- |
| Transport | Shared Memory | TCP/HTTP | Shared Memory | TCP |
| Serialization | Zero-Copy | Protobuf | Arrow IPC | RESP |
| Latency | ~10ns | ~1-10ms | ~100ns | ~1ms |
| Cross-Language | Any with mmap | Language-specific | C/Python/JS | Any with TCP |
| Hardware (FPGA) | Yes (MMIO) | No | No | No |
| Blocking | Non-blocking | Blocking | Non-blocking | Blocking |

## 10. Future Extensions

- **v1.1:** Multi-consumer broadcast channels (pub/sub pattern)
- **v1.2:** Encrypted payload planes with shared-key negotiation
- **v2.0:** Cross-machine Metropolitan via RDMA (Remote Direct Memory Access)
