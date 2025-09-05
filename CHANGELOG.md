# Changelog

All notable changes to LXTUI will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial public release
- Complete container lifecycle management (create, start, stop, restart, delete, clone)
- Real-time operation tracking with progress indicators
- Container creation wizard with image selection
- Direct shell execution into running containers
- Operations sidebar for monitoring concurrent operations
- Intuitive keyboard shortcuts with vim-style navigation
- Smart context-aware actions
- Robust error handling with retry logic
- Non-blocking async operations
- LXD REST API integration with WebSocket support

### Features
- **Container Management**
  - List all containers with status indicators
  - Start, stop, restart containers
  - Create new containers with guided wizard
  - Delete containers with confirmation
  - Clone existing containers
  - Execute shell commands in running containers

- **User Interface** 
  - Clean terminal-based interface
  - Real-time updates without blocking
  - Progress tracking for long-running operations
  - Contextual menus and shortcuts
  - Error dialogs with actionable suggestions
  - Operations sidebar for monitoring background tasks

- **Performance**
  - Async operation handling
  - Efficient terminal rendering
  - Minimal resource usage
  - Responsive user interface

### Technical
- Built with Ratatui for terminal UI
- Tokio async runtime for non-blocking operations
- Direct LXD REST API communication
- Comprehensive error handling and recovery
- Modular architecture for maintainability

## [0.1.0] - 2024-XX-XX

### Added
- Initial development version
- Core container management functionality
- Basic terminal user interface
- LXD integration via command line tools

---

## Release Process

1. Update version in `Cargo.toml`
2. Update this changelog
3. Create git tag: `git tag -a v1.0.0 -m "Release v1.0.0"`
4. Push tag: `git push origin v1.0.0`
5. GitHub Actions will automatically build and publish releases