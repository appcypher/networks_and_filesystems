# Network Interface Creator

A system daemon for managing TUN devices via a REST API, supporting both Linux and macOS systems.

## Features

- REST API for TUN interface management
- Automatic subnet allocation in 10.0.0.0/8 range
- Secure daemon operation with proper privilege handling
- Comprehensive logging system
- Platform-specific service management (systemd/launchd)

## Prerequisites

- Rust toolchain (1.70.0 or later)
- Root/Administrative privileges
- Platform-specific requirements:
  - Linux: systemd
  - macOS: launchd

## Installation

```bash
# Build and install
make install
```

The installation process:

1. Builds the release binary
2. Installs it to `/usr/local/bin/tun-daemon`
3. Sets up log files in `/var/log/`
4. Configures and starts the system service:
   - Linux: systemd service
   - macOS: launchd daemon

## Uninstallation

```bash
make uninstall
```

This will:

1. Stop the daemon service
2. Remove all installed files
3. Clean up system service configurations

## Usage

The daemon runs a REST API server on `localhost:3030` with the following endpoints:

### Create a TUN device

```bash
curl -X POST http://localhost:3030/tun -H "Content-Type: application/json" -d '{"name": "optional_name"}'
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

## Logging

Logs are split between:

- `/var/log/tun-daemon.log`: INFO, DEBUG, and TRACE level logs
- `/var/log/tun-daemon.err`: ERROR and WARN level logs

## Security

The daemon implements several security measures:

### Linux

- Runs as root with restricted capabilities
- Uses systemd security features:
  - Capability bounding (CAP_NET_ADMIN)
  - No new privileges
  - Protected system and home
  - Restricted address families
  - Namespace restrictions

### macOS

- Runs as root with wheel group
- Uses launchd's security features
- Proper file permissions for all components
