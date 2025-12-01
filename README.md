# Implementing a Ping in Rust

This project is a simplified implementation of the classic `ping` utility, written in Rust. The goal is to demonstrate low-level networking concepts (Raw Sockets), the ICMP protocol, and how to use the Rust language.

## Learning Objectives

1. How to create and manipulate sockets that allow direct access to network protocols (ICMP).
2. Structure of Echo Request and Echo Reply packets.
3. Usage of `libc`, `socket2`, error handling with `anyhow`, and byte manipulation.
4. Signal handling (Ctrl+C) and state sharing between threads.



## Key Concepts

### 1. Raw Sockets (`socket2`)

Unlike common TCP or UDP sockets, where the Operating System manages transport headers, **Raw Sockets** allow the application to build its own protocol headers.

In the code (`src/main.rs`), we use the `socket2` crate to create a `RAW` type socket:

```rust
let mut sock = Socket::new(
    Domain::IPV4,           // IPv4 Address Family (AF_INET)
    Type::from(3),          // SOCK_RAW (Value 3)
    Some(Protocol::ICMPV4), // ICMP Protocol (IPPROTO_ICMP)
)?;
```

> **Note**: Using Raw Sockets requires elevated privileges (Administrator on Windows or Root on Linux/macOS) because it allows sending arbitrary packets, which can be a security risk.

### 2. The ICMP Protocol

`ping` works by sending an **ICMP Echo Request** (Type 8) message and waiting for an **ICMP Echo Reply** (Type 0).

The basic structure of an ICMP packet that we assemble in `src/icmp.rs` is:

| Byte | Field | Description |
|---|---|---|
| 0 | Type | 8 for Request, 0 for Reply |
| 1 | Code | Always 0 for Echo |
| 2-3 | Checksum | Checksum to ensure integrity |
| 4-5 | Identifier | ID to correlate requests and replies (we use the PID) |
| 6-7 | Sequence | Sequential number to detect loss/ordering |
| 8+ | Payload | Arbitrary data (we use "pingrs-windows") |

### 3. Byte Order (Endianness)

Networks operate in **Big Endian** (Network Byte Order), while most CPUs (x86/x64) operate in **Little Endian**.
Rust forces us to be explicit about this. Note the use of `to_be_bytes()` and `from_be_bytes()`:

```rust
// When sending (icmp.rs)
pkt.extend_from_slice(&ident.to_be_bytes());

// When receiving (main.rs)
let r_id = u16::from_be_bytes([icmp[4], icmp[5]]);
```

### 4. Error Handling (`anyhow`)

Rust does not use exceptions. Functions that can fail return `Result<T, E>`.
We use the `anyhow` crate to facilitate error handling, allowing us to add context:

```rust
.context("Failed to create RAW socket. Check if running as Administrator.")?
```

The `?` operator propagates the error upwards if it occurs, keeping the code clean.

### 5. Concurrency and Signals (`ctrlc`)

To display statistics when the user presses `Ctrl+C`, we need a separate thread or a handler. We use the `ctrlc` crate.
To share the `running` flag between the handler (which runs in another thread) and the main loop, we use `Arc` (Atomic Reference Counting) and `AtomicBool`:

```rust
let running = Arc::new(AtomicBool::new(true));
// ...
// In the main loop:
if !running.load(Ordering::SeqCst) { break; }
```

---

## Code Structure

*   **`src/main.rs`**: The entry point.
    *   Parses arguments.
    *   Configures the socket and Ctrl+C handler.
    *   Main loop: Sends Ping -> Waits for Pong -> Calculates RTT -> Sleeps.
    *   Displays final statistics.
*   **`src/icmp.rs`**: Protocol-specific logic.
    *   `build_echo_request`: Assembles the packet byte vector.
    *   `checksum`: Implements the checksum algorithm (1's complement).
*   **`src/args.rs`**: Manual command-line argument parsing (`-c <count>`, `<ip>`).

---

## How to Run

Since this project uses Raw Sockets, it requires elevated privileges to create the socket.

### Windows
You **MUST** run the terminal as **Administrator**.

1.  Open PowerShell or CMD as Administrator.
2.  Run:
    ```bash
    cargo run -- 8.8.8.8
    ```

### Linux
You have two options:

**Option 1: Run with `sudo` (Easiest)**
```bash
sudo cargo run -- 8.8.8.8
```

**Option 2: Grant Capabilities (Advanced)**
You can grant the `CAP_NET_RAW` capability to the binary to run without `sudo`.
```bash
# Build first
cargo build --release
# Grant capability
sudo setcap cap_net_raw+ep target/release/pingrs
# Run normally
./target/release/pingrs 8.8.8.8
```