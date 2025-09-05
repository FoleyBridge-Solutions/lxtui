# LXTUI Examples

This directory contains example configurations and usage patterns for LXTUI.

## Environment Variables

### Debug Logging

Enable detailed logging for troubleshooting:

```bash
# Basic debug logging
RUST_LOG=debug lxtui

# Specific module logging
RUST_LOG=lxtui::lxd_api=trace lxtui

# Log to file
RUST_LOG=debug lxtui 2> lxtui.log
```

### Log Levels

- `error` - Only errors
- `warn` - Warnings and errors  
- `info` - General information
- `debug` - Detailed debugging info
- `trace` - Very verbose debugging

## LXD Configuration Examples

### Basic LXD Setup

```bash
# Initialize LXD with default settings
sudo lxd init --auto

# Create a storage pool
lxc storage create default dir

# Create a network bridge
lxc network create lxdbr0

# Set up a profile
lxc profile create default
lxc profile device add default root disk path=/ pool=default
lxc profile device add default eth0 nic network=lxdbr0
```

### Advanced LXD Setup

```bash
# Initialize with custom settings
sudo lxd init

# Custom storage with ZFS
lxc storage create zfs-pool zfs source=/dev/sdb

# Custom network with specific subnet
lxc network create custom-net \
  ipv4.address=10.0.100.1/24 \
  ipv4.nat=true \
  ipv6.address=none

# Profile for development containers
lxc profile create dev
lxc profile set dev limits.cpu=4
lxc profile set dev limits.memory=8GB
lxc profile device add dev root disk path=/ pool=zfs-pool
lxc profile device add dev eth0 nic network=custom-net
```

## Common Usage Patterns

### Container Templates

LXTUI works well with pre-configured container profiles:

```bash
# Create profiles for different use cases
lxc profile create webserver
lxc profile set webserver limits.memory=2GB
lxc profile device add webserver http proxy \
  listen=tcp:0.0.0.0:80 \
  connect=tcp:127.0.0.1:8080

lxc profile create database  
lxc profile set database limits.memory=4GB
lxc profile set database limits.cpu=2
lxc profile device add database data disk \
  source=/srv/database \
  path=/var/lib/mysql
```

### Batch Operations

While LXTUI focuses on interactive management, you can prepare environments:

```bash
# Create multiple containers
for i in {1..5}; do
  lxc launch ubuntu:22.04 web-$i --profile webserver
done

# Then manage them interactively with LXTUI
lxtui
```

## Troubleshooting Examples

### Connection Issues

```bash
# Check LXD status
systemctl status snap.lxd.daemon

# Verify socket permissions
ls -la /var/snap/lxd/common/lxd/unix.socket

# Test LXD API
curl --unix-socket /var/snap/lxd/common/lxd/unix.socket \
     http://localhost/1.0
```

### Permission Problems

```bash
# Add user to lxd group
sudo usermod -a -G lxd $USER

# Verify group membership
groups $USER

# Check if you need to log out/in
id $USER
```

### Performance Monitoring

```bash
# Monitor with debug logging
RUST_LOG=info lxtui 2> performance.log &
PID=$!

# Monitor resource usage
top -p $PID
```

## Integration Examples

### Shell Scripts

```bash
#!/bin/bash
# launch-dev-env.sh
# Sets up development environment then launches LXTUI

# Ensure LXD is running
sudo systemctl start snap.lxd.daemon

# Create development profile if it doesn't exist
if ! lxc profile show dev >/dev/null 2>&1; then
    lxc profile create dev
    lxc profile set dev limits.memory=4GB
    lxc profile set dev limits.cpu=4
fi

# Launch LXTUI
exec lxtui
```

### Systemd Service

```ini
# /etc/systemd/user/lxtui.service
[Unit]
Description=LXTUI Container Management
After=snap.lxd.daemon.service

[Service]
Type=simple
ExecStart=/usr/local/bin/lxtui
Environment=RUST_LOG=warn
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

## Development Examples

### Custom Image Sources

```bash
# Add custom image server
lxc remote add my-images https://images.example.com \
  --protocol=simplestreams --public

# Use in LXTUI container creation wizard
# Images from my-images will appear in the selection list
```

### Network Configuration

```bash
# Create isolated network for testing
lxc network create testnet \
  ipv4.address=192.168.100.1/24 \
  ipv4.dhcp=true \
  ipv4.nat=false \
  ipv6.address=none

# Create profile using test network
lxc profile create testing
lxc profile device add testing eth0 nic network=testnet
```

These examples demonstrate common patterns and configurations that work well with LXTUI's interactive container management approach.