# LXTUI

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/lxtui.svg)](https://crates.io/crates/lxtui)

A modern, fast, and intuitive terminal user interface for managing LXC/LXD containers. Built with Rust and designed for developers who prefer command-line workflows.

![LXTUI Demo](docs/demo.gif)

## ✨ Features

- **🚀 Fast & Responsive** - Async operations with real-time updates
- **📱 Modern TUI** - Clean, intuitive interface built with Ratatui
- **🔄 Real-time Operations** - Live progress tracking for container operations
- **⌨️ Keyboard-first** - Vim-style navigation with intuitive shortcuts
- **🎯 Smart Actions** - Context-aware operations (start/stop toggle)
- **🔍 Operation Monitoring** - Track running operations with detailed status
- **🛡️ Robust Error Handling** - Graceful error handling with retry logic
- **📊 Container Overview** - View status, resource usage, and details at a glance

## 📦 Installation

### From Cargo (Recommended)

```bash
cargo install lxtui
```

### From Source

```bash
git clone https://github.com/yourusername/lxtui.git
cd lxtui
cargo build --release
sudo cp target/release/lxtui /usr/local/bin/
```

### Package Managers

#### Arch Linux (AUR)
```bash
yay -S lxtui
```

#### Ubuntu/Debian (Coming Soon)
```bash
# Will be available via apt once published
```

## 🚀 Quick Start

1. **Ensure LXD is installed and running:**
   ```bash
   # Ubuntu/Debian
   sudo apt install lxd
   sudo lxd init
   
   # Arch Linux
   sudo pacman -S lxd
   sudo lxd init
   ```

2. **Add your user to the lxd group:**
   ```bash
   sudo usermod -a -G lxd $USER
   newgrp lxd
   ```

3. **Run LXTUI:**
   ```bash
   lxtui
   ```

## ⌨️ Key Bindings

### Main Container List
- **↑/↓** or **j/k** - Navigate containers
- **Enter** - Open container actions menu
- **Space** - Open system menu
- **s** - Start selected container (quick action)
- **S** - Stop selected container (quick action)
- **d** - Delete selected container (quick action)
- **n** - Create new container
- **r/R** - Refresh container list
- **o/O** - Toggle operations sidebar
- **?/h** - Show help
- **q/Q** - Quit

### Container Actions Menu
- **Enter** - Smart action (Start if stopped, Stop if running)
- **1** - Start container
- **2** - Stop container  
- **3** - Restart container
- **4** - Delete container
- **5** - Clone container
- **e** - Execute shell (container must be running)
- **Esc** - Close menu

### System Menu
- **1/r** - Refresh container list
- **2/l** - Check/start LXD service
- **3/n** - Create new container
- **4/o** - Toggle operations sidebar
- **5/h** - Show help
- **6/q** - Quit application
- **Esc** - Close menu

For complete keybindings, see [KEYBINDINGS.md](KEYBINDINGS.md).

## 🛠️ System Requirements

- **Operating System:** Linux (Ubuntu 20.04+, Debian 11+, Arch Linux, Fedora 35+)
- **LXD Version:** 4.0+ (5.0+ recommended)
- **Terminal:** Any modern terminal emulator with 256+ colors
- **Memory:** ~10MB RAM
- **Storage:** ~15MB disk space

## ⚙️ Configuration

LXTUI works out of the box with standard LXD installations. Configuration options:

### Environment Variables

- `RUST_LOG` - Set logging level (`off`, `error`, `warn`, `info`, `debug`, `trace`)
  ```bash
  RUST_LOG=debug lxtui  # Enable debug logging
  ```

- `LXD_SOCKET` - Custom LXD socket path (defaults to `/var/lib/lxd/unix.socket`)
  ```bash
  LXD_SOCKET=/custom/path/unix.socket lxtui
  ```

### LXD Remote Configuration

LXTUI supports LXD remote servers. Configure remotes using the LXD client:

```bash
lxc remote add myserver https://server.example.com:8443
lxtui --remote myserver
```

## 🏗️ Architecture

LXTUI is built with a modern async architecture:

- **Frontend:** Ratatui for the terminal UI
- **Backend:** Tokio async runtime
- **LXD Communication:** RESTful API over Unix sockets
- **State Management:** Centralized app state with real-time updates
- **Error Handling:** Comprehensive error types with retry logic

### Key Components

- `src/app.rs` - Main application state and logic
- `src/ui.rs` - Terminal UI rendering
- `src/lxd_api.rs` - LXD API client and operations
- `src/lxc.rs` - Container management and state tracking

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/yourusername/lxtui.git
   cd lxtui
   ```

2. **Install Rust and dependencies:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup update
   ```

3. **Run in development mode:**
   ```bash
   RUST_LOG=debug cargo run
   ```

4. **Run tests:**
   ```bash
   cargo test
   ```

5. **Format code:**
   ```bash
   cargo fmt
   ```

6. **Lint code:**
   ```bash
   cargo clippy
   ```

### Project Structure

```
lxtui/
├── src/
│   ├── main.rs          # Application entry point
│   ├── app.rs           # Main application logic
│   ├── ui.rs            # Terminal UI components
│   ├── lxd_api.rs       # LXD API client
│   └── lxc.rs           # Container operations
├── tests/               # Integration tests
├── docs/                # Documentation
├── .github/workflows/   # CI/CD pipelines
└── README.md
```

## 🐛 Troubleshooting

### Common Issues

**1. "Failed to connect to LXD"**
```bash
# Check if LXD is running
sudo systemctl status lxd

# Start LXD if needed
sudo systemctl start lxd

# Verify your user is in the lxd group
groups $USER
```

**2. "Permission denied" errors**
```bash
# Add user to lxd group and refresh
sudo usermod -a -G lxd $USER
newgrp lxd
```

**3. "No containers found"**
```bash
# Verify LXD is initialized
lxc profile list

# If not initialized
sudo lxd init
```

**4. Terminal display issues**
```bash
# Try setting terminal type
export TERM=xterm-256color
lxtui
```

### Debug Mode

Enable debug logging for troubleshooting:

```bash
RUST_LOG=debug lxtui 2> lxtui.log
```

### Reporting Issues

Please report issues on [GitHub Issues](https://github.com/yourusername/lxtui/issues) with:

1. Your operating system and version
2. LXD version (`lxd --version`)
3. LXTUI version (`lxtui --version`)
4. Steps to reproduce the issue
5. Debug logs if applicable

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Ratatui](https://github.com/ratatui-org/ratatui) - Excellent TUI framework
- [LXD Team](https://linuxcontainers.org/lxd/) - Amazing container runtime
- [Tokio](https://tokio.rs/) - Async runtime for Rust
- The Rust community for incredible tooling and support

## 🔗 Links

- **Homepage:** [https://github.com/yourusername/lxtui](https://github.com/yourusername/lxtui)
- **Documentation:** [https://docs.rs/lxtui](https://docs.rs/lxtui)
- **Crates.io:** [https://crates.io/crates/lxtui](https://crates.io/crates/lxtui)
- **Changelog:** [CHANGELOG.md](CHANGELOG.md)
- **Contributing:** [CONTRIBUTING.md](CONTRIBUTING.md)

---

**Made with ❤️ by the LXTUI contributors**

*Streamline your container workflow with the power of the terminal.*