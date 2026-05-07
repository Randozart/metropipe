/**
 * Metropolitan FFI Client - JavaScript/Node.js
 * Zero-copy shared memory communication with Metro Daemon services.
 *
 * Usage:
 *   const { MetroClient } = require('./metro.js');
 *
 *   const client = new MetroClient('WeatherApi');
 *   const result = await client.request(payload);
 */

const fs = require('fs');
const path = require('path');

// Status word constants
const STATUS_IDLE = 0;
const STATUS_CONSUMER_REQ = 1;
const STATUS_PROVIDER_ACK = 2;
const STATUS_PROVIDER_RES = 3;
const STATUS_ERROR = 4;

// Header offsets
const HEADER_SIZE = 32;
const OFFSET_STATUS = 0;
const OFFSET_CAS_LOCK = 4;
const OFFSET_PAYLOAD_SIZE = 8;
const OFFSET_MAX_CAPACITY = 12;
const OFFSET_ERROR_CODE = 16;
const OFFSET_PAYLOAD = 32;

class MetroError extends Error {
    constructor(message) {
        super(message);
        this.name = 'MetroError';
    }
}

class MetroTimeoutError extends MetroError {
    constructor(message) {
        super(message);
        this.name = 'MetroTimeoutError';
    }
}

class MetroChannel {
    constructor(shmPath) {
        this.shmPath = shmPath;
        this.buffer = null;
        this.header = null;
        this.payload = null;
        this._open();
    }

    _open() {
        if (!fs.existsSync(this.shmPath)) {
            throw new MetroError(`Shared memory not found: ${this.shmPath}`);
        }
        const size = fs.statSync(this.shmPath).size;
        this.buffer = new SharedArrayBuffer(size);
        this.header = new Int32Array(this.buffer, 0, 8);
        this.payload = new Uint8Array(this.buffer, OFFSET_PAYLOAD);

        // Load initial content from file into SharedArrayBuffer
        const fileData = fs.readFileSync(this.shmPath);
        const view = new Uint8Array(this.buffer);
        view.set(fileData);
    }

    _readStatus() {
        return Atomics.load(this.header, OFFSET_STATUS / 4);
    }

    _writeStatus(value) {
        Atomics.store(this.header, OFFSET_STATUS / 4, value);
    }

    _readPayloadSize() {
        return Atomics.load(this.header, OFFSET_PAYLOAD_SIZE / 4);
    }

    _writePayloadSize(size) {
        Atomics.store(this.header, OFFSET_PAYLOAD_SIZE / 4, size);
    }

    async waitIdle(timeoutMs = 5000) {
        const start = Date.now();
        while (this._readStatus() !== STATUS_IDLE) {
            if (Date.now() - start > timeoutMs) {
                throw new MetroTimeoutError('Timed out waiting for IDLE state');
            }
            await new Promise(r => setTimeout(r, 1));
        }
    }

    sendRequest(payload) {
        this.waitIdle();
        this.payload.set(payload);
        this._writePayloadSize(payload.length);
        this._writeStatus(STATUS_CONSUMER_REQ);
        Atomics.notify(this.header, OFFSET_STATUS / 4, 1);
    }

    async waitResponse(timeoutMs = 5000) {
        const start = Date.now();
        while (true) {
            const status = this._readStatus();
            if (status === STATUS_PROVIDER_RES) {
                const size = this._readPayloadSize();
                const result = this.payload.slice(0, size);
                this._writeStatus(STATUS_IDLE);
                return result;
            }
            if (status === STATUS_ERROR) {
                const code = Atomics.load(this.header, OFFSET_ERROR_CODE / 4);
                throw new MetroError(`Provider error: code ${code}`);
            }
            if (Date.now() - start > timeoutMs) {
                throw new MetroTimeoutError('Provider did not respond');
            }
            await new Promise(r => setTimeout(r, 1));
        }
    }

    async request(payload, timeoutMs = 5000) {
        this.sendRequest(payload);
        return this.waitResponse(timeoutMs);
    }

    close() {
        // SharedArrayBuffer is garbage collected
        this.buffer = null;
        this.header = null;
        this.payload = null;
    }
}

class MetroClient {
    constructor(serviceName, shmDir = '/dev/shm') {
        this.serviceName = serviceName;
        this.shmPath = path.join(shmDir, `metro_${serviceName}`);
        this.specPath = `${this.shmPath}_spec.json`;
        this.channel = new MetroChannel(this.shmPath);
        this.spec = this._loadSpec();
    }

    _loadSpec() {
        try {
            const data = fs.readFileSync(this.specPath, 'utf8');
            return JSON.parse(data);
        } catch {
            return {};
        }
    }

    async request(payload, timeoutMs = 5000) {
        return this.channel.request(payload, timeoutMs);
    }

    close() {
        this.channel.close();
    }
}

class MetroBroker {
    constructor(shmDir = '/dev/shm') {
        this.shmDir = shmDir;
    }

    registerService(name, capacity = 4096) {
        const shmPath = path.join(this.shmDir, `metro_${name}`);
        if (!fs.existsSync(shmPath)) {
            const buffer = Buffer.alloc(32 + capacity, 0);
            fs.writeFileSync(shmPath, buffer);
        }
        return shmPath;
    }

    lookupService(name) {
        const shmPath = path.join(this.shmDir, `metro_${name}`);
        if (fs.existsSync(shmPath)) {
            return shmPath;
        }
        return null;
    }

    listServices() {
        try {
            const files = fs.readdirSync(this.shmDir);
            return files
                .filter(f => f.startsWith('metro_') && !f.endsWith('_spec.json'))
                .map(f => f.slice(6));
        } catch {
            return [];
        }
    }
}

module.exports = { MetroChannel, MetroClient, MetroBroker, MetroError, MetroTimeoutError };
