# TUN Device Management Daemon

A system daemon for managing TUN devices via a REST API. Supports both Linux and macOS.

## Prerequisites

- Rust toolchain (1.70.0 or later)
- Root privileges for installation and running
- Platform-specific requirements:
  - Linux: systemd
  - macOS: launchd

## Installation

```bash
# From the root directory
make install
```

The installation process will:

1. Build the release binary
2. Install it to `/usr/local/bin/tun-daemon`
3. Create and configure log files:
   - `/var/log/tun-daemon.log` for normal operation logs
   - `/var/log/tun-daemon.err` for error logs
4. Create and configure PID file at `/var/run/tun-daemon.pid`
5. Install and start the system service:
   - Linux: systemd service with security restrictions
   - macOS: launchd daemon

## Uninstallation

```bash
# From the root directory
make uninstall
```

This will:

1. Stop the daemon (gracefully or force if needed)
2. Remove all installed files
3. Clean up system configurations

## API Usage

The daemon runs a REST API server on `localhost:3030` with the following endpoints:

### Create a TUN device

```bash
curl -X POST http://localhost:3030/tun -H "Content-Type: application/json" -d '{"name": "optional_name"}'
```

The name parameter is optional. If not provided, the system will assign a name automatically.

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
  - Capability bounding (CAP_NET_ADMIN only)
  - No new privileges
  - Protected system and home
  - Restricted address families (AF_INET, AF_INET6, AF_UNIX)
  - Restricted namespaces

### macOS

- Runs as root with wheel group
- Uses launchd's security features
- Proper file permissions (644 for logs/pid, 755 for binary)

## Logging

The daemon uses a comprehensive logging system:

- `/var/log/tun-daemon.log`:

  - INFO: Normal operational events
  - DEBUG: Detailed debugging information
  - TRACE: Very detailed debugging information

- `/var/log/tun-daemon.err`:
  - ERROR: Critical issues that need attention
  - WARN: Warning conditions

Each log entry includes:

- Timestamp
- Log level
- Thread ID
- File name and line number
- Message
