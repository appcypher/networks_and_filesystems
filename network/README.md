# TUN Device Management Daemon

A system daemon for managing TUN devices via a REST API. Supports both Linux and macOS.

## Prerequisites

- Rust toolchain (1.70.0 or later)
- Root privileges for installation and running
- Linux: systemd
- macOS: launchd

## Installation

```bash
# Clone the repository
git clone <repository-url>

# Build and install from the root directory
make install
```

The installation process will:

1. Build the binary
2. Install it to `/usr/local/bin/tun-daemon`
3. Install and start the system service (systemd on Linux, launchd on macOS)

## Uninstallation

```bash
# From the root directory
make uninstall
```

## Usage

The daemon runs a REST API server on `localhost:3030` with the following endpoints:

### Create a TUN device

```bash
curl -X POST http://localhost:3030/tun -H "Content-Type: application/json" -d '{}'
```

Response:

```json
{
  "name": "utun3",
  "ip_addr": "10.0.0.1",
  "netmask": "255.255.255.0",
  "broadcast": "10.0.0.255"
}
```

### List TUN devices

```bash
curl http://localhost:3030/tun
```

Response:

```json
[
  {
    "name": "utun3",
    "ip_addr": "10.0.0.1",
    "netmask": "255.255.255.0",
    "broadcast": "10.0.0.255"
  }
]
```

## Security

The daemon runs with root privileges but implements several security measures:

### Linux

- Uses systemd's security features:
  - Capability bounding and ambient capabilities
  - No new privileges
  - Protected system and home
  - Restricted address families and namespaces

### macOS

- Runs as root but in a restricted environment
- Uses launchd's security features

## Logging

Logs can be found at:

- Linux: Use `journalctl -u tun-daemon`
- macOS: Check `/var/log/tun-daemon.log` and `/var/log/tun-daemon.err`
