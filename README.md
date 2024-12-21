# Network Interface Creator

This is a Rust project that demonstrates how to create TUN interfaces on macOS (utun) and Linux (tun) systems.

## Features

- Platform-specific TUN interface creation
- Automatic privilege elevation using platform-appropriate methods
- Simple configuration of network interfaces

## Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- On macOS: Administrative privileges
- On Linux: Administrative privileges and `pkexec` (usually pre-installed)

## Building

```bash
cargo build --release
```

## Running

The program will automatically request administrative privileges when needed:

```bash
cargo run --release
```

### Platform-specific Details

#### macOS
- Creates a `utun7` interface
- Configures IP address 10.0.0.1 with remote endpoint 10.0.0.2
- Uses `osascript` for privilege elevation

#### Linux
- Creates a `tun0` interface
- Configures IP address 10.0.0.1/24
- Uses `pkexec` for privilege elevation

## Note

This is a demonstration project. In production environments, you should implement proper error handling and security measures.
