# Network Interface Creator

A system daemon for managing network interfaces via REST APIs, supporting both Linux and macOS systems.

## Features

- REST APIs for network interface management:
  - TUN interface creation and management
  - Subnet configuration and management
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

### TUN Daemon

```bash
# Build and install TUN daemon
make install_tun
```

The installation process:

1. Builds the release binary
2. Installs it to `/usr/local/bin/tun-daemon`
3. Sets up log files in `/var/log/`
4. Configures and starts the system service:
   - Linux: systemd service
   - macOS: launchd daemon

### Subnet Daemon

```bash
# Build and install Subnet daemon
make install
```

The installation process:

1. Builds the release binary
2. Installs it to `/usr/local/bin/subnet-daemon`
3. Sets up log files in `/var/log/`
4. Configures and starts the system service:
   - Linux: systemd service
   - macOS: launchd daemon

## Uninstallation

### TUN Daemon

```bash
make uninstall_tun
```

### Subnet Daemon

```bash
make uninstall
```

These commands will:

1. Stop the respective daemon service
2. Remove all installed files
3. Clean up system service configurations

## Usage

### TUN Interface Management

The TUN daemon runs a REST API server on `localhost:3030` with the following endpoints:

#### Create a TUN device

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

#### List TUN devices

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

### Subnet Management

The subnet daemon runs a REST API server on `localhost:3031` with the following endpoints:

#### Create a subnet

```bash
curl -X POST http://localhost:3031/subnet -H "Content-Type: application/json" -d '{"cidr": "10.1.0.0/24"}'
```

Response:

```json
{
  "network": "10.1.0.0/24",
  "interface": "lo0:0"  # on macOS, or a dummy interface name on Linux
}
```

#### List subnets

```bash
curl http://localhost:3031/subnet
```

Response:

```json
[
  {
    "network": "10.1.0.0/24",
    "interface": "lo0:0"
  }
]
```

#### Delete a subnet

```bash
curl -X DELETE http://localhost:3031/subnet/10.1.0.0%2F24
```

## Logging

Logs are split between:

### TUN Daemon

- `/var/log/tun_daemon.log`: INFO, DEBUG, and TRACE level logs
- `/var/log/tun_daemon.err`: ERROR and WARN level logs

### Subnet Daemon

- `/var/log/subnet_daemon.log`: INFO, DEBUG, and TRACE level logs
- `/var/log/subnet_daemon.err`: ERROR and WARN level logs

## Security

The daemons implement several security measures:

### Linux

- Run as root with restricted capabilities
- Use systemd security features:
  - Capability bounding (CAP_NET_ADMIN)
  - No new privileges
  - Protected system and home
  - Restricted address families
  - Namespace restrictions

### macOS

- Run as root with wheel group
- Use launchd's security features
- Proper file permissions for all components
