"""
metropipe — Universal Language Binder
Python client — zero-copy shared memory IPC via the Metropolitan protocol.

Usage:
    from metro import MetroClient

    # Connect to a service
    client = MetroClient("WeatherApi")

    # Send a request
    client.send_request(city_data_bytes)

    # Wait for response
    result = client.wait_response(timeout_ms=5000)
"""

import mmap
import struct
import time
import os
import json
from typing import Optional


class MetroError(Exception):
    """Base exception for Metropolitan FFI errors."""
    pass


class MetroTimeoutError(MetroError):
    """Raised when a provider does not respond within the timeout."""
    pass


class MetroChannel:
    """Low-level Metropolitan shared memory channel."""

    STATUS_IDLE = 0
    STATUS_CONSUMER_REQ = 1
    STATUS_PROVIDER_ACK = 2
    STATUS_PROVIDER_RES = 3
    STATUS_ERROR = 4

    HEADER_SIZE = 32
    OFFSET_STATUS = 0
    OFFSET_CAS_LOCK = 4
    OFFSET_PAYLOAD_SIZE = 8
    OFFSET_MAX_CAPACITY = 12
    OFFSET_ERROR_CODE = 16
    OFFSET_PAYLOAD = 32

        self.shm_path = shm_path
        self._fd = None
        self._mmap = None
        self._open()

        if not os.path.exists(self.shm_path):
            raise MetroError(f"Shared memory not found: {self.shm_path}")
        self._fd = open(self.shm_path, "r+b")
        self._mmap = mmap.mmap(self._fd.fileno(), 0)

        if self._mmap:
            self._mmap.close()
            self._mmap = None
        if self._fd:
            self._fd.close()
            self._fd = None

        return self

        self.close()

        return struct.unpack_from("<I", self._mmap, self.OFFSET_STATUS)[0]

        struct.pack_into("<I", self._mmap, self.OFFSET_STATUS, value)

        return struct.unpack_from("<I", self._mmap, self.OFFSET_PAYLOAD_SIZE)[0]

        struct.pack_into("<I", self._mmap, self.OFFSET_PAYLOAD_SIZE, size)

        start = time.monotonic()
        while True:
            if self._read_status() == self.STATUS_IDLE:
                return
            if (time.monotonic() - start) * 1000 > timeout_ms:
                raise MetroTimeoutError("Timed out waiting for IDLE state")
            time.sleep(0.001)

        self.wait_idle()
        size = len(payload)
        self._mmap[self.OFFSET_PAYLOAD:self.OFFSET_PAYLOAD + size] = payload
        self._write_payload_size(size)
        self._write_status(self.STATUS_CONSUMER_REQ)

        start = time.monotonic()
        while True:
            status = self._read_status()
            if status == self.STATUS_PROVIDER_RES:
                size = self._read_payload_size()
                result = bytes(self._mmap[self.OFFSET_PAYLOAD:self.OFFSET_PAYLOAD + size])
                self._write_status(self.STATUS_IDLE)
                return result
            if status == self.STATUS_ERROR:
                code = struct.unpack_from("<I", self._mmap, self.OFFSET_ERROR_CODE)[0]
                raise MetroError(f"Provider error: code {code}")
            if (time.monotonic() - start) * 1000 > timeout_ms:
                raise MetroTimeoutError("Provider did not respond")
            time.sleep(0.001)

        self.send_request(payload)
        return self.wait_response(timeout_ms)


class MetroClient:
    """High-level client for a specific Metropolitan service."""

        self.service_name = service_name
        self.shm_path = os.path.join(shm_dir, f"metro_{service_name}")
        self.spec_path = f"{self.shm_path}_spec.json"
        self.channel = MetroChannel(self.shm_path)
        self.spec = self._load_spec()

        if os.path.exists(self.spec_path):
            with open(self.spec_path) as f:
                return json.load(f)
        return {}

        return self.channel.request(payload, timeout_ms)

        self.channel.close()

        return self

        self.close()


class MetroBroker:
    """Client for the Metro Daemon broker itself."""

    CMD_REGISTER = 1
    CMD_LOOKUP = 2
    CMD_DEREGISTER = 3
    CMD_STATUS = 4
    CMD_SHUTDOWN = 5

        self.shm_dir = shm_dir

        shm_path = os.path.join(self.shm_dir, f"metro_{name}")
        if not os.path.exists(shm_path):
            with open(shm_path, "wb") as f:
                f.write(b'\x00' * (32 + capacity))
        return shm_path

        shm_path = os.path.join(self.shm_dir, f"metro_{name}")
        if os.path.exists(shm_path):
            return shm_path
        return None

        services = []
        if os.path.exists(self.shm_dir):
            for f in os.listdir(self.shm_dir):
                if f.startswith("metro_") and not f.endswith("_spec.json"):
                    services.append(f[6:])
        return services
