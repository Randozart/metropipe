/*
 * metropipe - Universal Language Binder
 * C client — zero-copy shared memory IPC via the Metropolitan protocol.
 *
 * Usage:
 *   #include "metropipe.h"
 *
 *   MetroChannel ch;
 *   metro_channel_open(&ch, "/dev/shm/metro_WeatherApi");
 *   uint8_t request[] = "New York";
 *   metro_channel_send(&ch, request, sizeof(request));
 *   uint8_t response[1024];
 *   int len = metro_channel_recv(&ch, response, sizeof(response), 5000);
 *   metro_channel_close(&ch);
 */

#ifndef METROPIPE_H
#define METROPIPE_H

#include <stdint.h>
#include <stddef.h>
#include <stdatomic.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===== Status Word Constants ===== */
#define METRO_STATUS_IDLE         0
#define METRO_STATUS_CONSUMER_REQ 1
#define METRO_STATUS_PROVIDER_ACK 2
#define METRO_STATUS_PROVIDER_RES 3
#define METRO_STATUS_ERROR        4

/* ===== Header Layout ===== */
#define METRO_HEADER_SIZE     32
#define METRO_OFFSET_STATUS   0
#define METRO_OFFSET_CAS_LOCK 4
#define METRO_OFFSET_SIZE     8
#define METRO_OFFSET_CAPACITY 12
#define METRO_OFFSET_ERROR    16
#define METRO_OFFSET_PAYLOAD  32

/* ===== Error Codes ===== */
#define METRO_OK              0
#define METRO_ERR_TIMEOUT    -1
#define METRO_ERR_NOT_FOUND  -2
#define METRO_ERR_OVERFLOW   -3
#define METRO_ERR_PROVIDER   -4

/* ===== Channel Structure ===== */
typedef struct {
    volatile uint32_t *header;    /* Pointer to 32-byte header */
    volatile uint8_t  *payload;   /* Pointer to payload region */
    size_t capacity;              /* Maximum payload size */
    int fd;                       /* Shared memory file descriptor */
    char path[256];               /* Shared memory path */
} MetroChannel;

/* ===== Core API ===== */

/**
 * Open a Metropolitan shared memory channel.
 * Returns METRO_OK on success, negative error code on failure.
 */
int metro_channel_open(MetroChannel *ch, const char *shm_path);

/**
 * Close a Metropolitan channel and release resources.
 */
void metro_channel_close(MetroChannel *ch);

/**
 * Wait for the channel to be in IDLE state.
 * Returns METRO_OK on success, METRO_ERR_TIMEOUT on timeout.
 */
int metro_wait_idle(MetroChannel *ch, int timeout_ms);

/**
 * Send a request through the channel.
 * Writes payload to shared memory and signals the provider.
 * Returns METRO_OK on success, negative error code on failure.
 */
int metro_channel_send(MetroChannel *ch, const uint8_t *data, size_t len);

/**
 * Receive a response from the channel.
 * Waits for PROVIDER_RES, copies result to out buffer.
 * Returns number of bytes received, or negative error code.
 */
int metro_channel_recv(MetroChannel *ch, uint8_t *out, size_t max_len, int timeout_ms);

/**
 * Send a request and wait for response (synchronous RPC).
 * Returns number of bytes in response, or negative error code.
 */
int metro_channel_request(MetroChannel *ch, const uint8_t *req, size_t req_len,
                          uint8_t *resp, size_t resp_max, int timeout_ms);

/* ===== Atomic Operations ===== */

/**
 * Read the status word atomically.
 */
static inline uint32_t metro_read_status(MetroChannel *ch) {
    return atomic_load_explicit((_Atomic uint32_t *)&ch->header[METRO_OFFSET_STATUS / 4],
                                memory_order_seq_cst);
}

/**
 * Write the status word atomically.
 */
static inline void metro_write_status(MetroChannel *ch, uint32_t value) {
    atomic_store_explicit((_Atomic uint32_t *)&ch->header[METRO_OFFSET_STATUS / 4],
                          value, memory_order_seq_cst);
}

/**
 * Atomic compare-and-swap on the status word.
 * Returns the previous value.
 */
static inline uint32_t metro_cas_status(MetroChannel *ch, uint32_t expected, uint32_t new_val) {
    _Atomic uint32_t *ptr = (_Atomic uint32_t *)&ch->header[METRO_OFFSET_STATUS / 4];
    uint32_t old = expected;
    atomic_compare_exchange_weak_explicit(ptr, &old, new_val,
                                          memory_order_seq_cst, memory_order_seq_cst);
    return old;
}

/**
 * Read the payload size atomically.
 */
static inline uint32_t metro_read_size(MetroChannel *ch) {
    return atomic_load_explicit((_Atomic uint32_t *)&ch->header[METRO_OFFSET_SIZE / 4],
                                memory_order_seq_cst);
}

/* ===== Broker API ===== */

/**
 * Register a new service with the Metro Daemon.
 * Creates the shared memory file and initializes the header.
 * Returns METRO_OK on success.
 */
int metro_broker_register(const char *service_name, size_t capacity);

/**
 * Look up a service by name.
 * Returns the shared memory path, or NULL if not found.
 */
const char *metro_broker_lookup(const char *service_name);

/**
 * List all registered services.
 * Fills the names array (up to max_count entries).
 * Returns the number of services found.
 */
int metro_broker_list(char names[][64], int max_count);

/* ===== Convenience Macros ===== */

#define METRO_CHANNEL_INIT(name) \
    MetroChannel name = { .header = NULL, .payload = NULL, .capacity = 0, .fd = -1 }

#define METRO_SERVICE_PATH(dir, name) \
    dir "/metro_" name

#ifdef __cplusplus
}
#endif

#endif /* METROPIPE_H */
