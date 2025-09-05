# Contributing to LXTUI

Thank you for your interest in contributing to LXTUI! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- Rust 1.70+ 
- LXD installed and configured
- Git

### Development Setup

1. Fork and clone the repository:
```bash
git clone https://github.com/yourusername/lxtui.git
cd lxtui
```

2. Build the project:
```bash
cargo build
```

3. Run tests:
```bash
cargo test
```

4. Run the application:
```bash
cargo run
```

## Development Workflow

### Branch Strategy

- `main` - Stable release branch
- `develop` - Development branch for new features
- `feature/feature-name` - Feature branches
- `fix/issue-description` - Bug fix branches

### Making Changes

1. Create a feature branch from `develop`:
```bash
git checkout develop
git pull origin develop
git checkout -b feature/your-feature-name
```

2. Make your changes following the coding standards
3. Add tests for new functionality
4. Ensure all tests pass:
```bash
cargo test
```

5. Run clippy for linting:
```bash
cargo clippy -- -D warnings
```

6. Format your code:
```bash
cargo fmt
```

7. Commit your changes with clear messages
8. Push and create a pull request

## Coding Standards

### Rust Style

- Follow standard Rust formatting (`cargo fmt`)
- Use `clippy` recommendations (`cargo clippy`)
- Write comprehensive documentation for public APIs
- Include unit tests for new functionality

### Code Structure

- Keep functions focused and small
- Use meaningful variable and function names  
- Add comments for complex logic
- Follow existing patterns in the codebase

### Error Handling

- Use `anyhow::Result` for functions that can fail
- Provide meaningful error messages
- Log appropriate information for debugging
- Handle errors gracefully in the UI

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

### Writing Tests

- Write unit tests for core logic
- Test error conditions
- Use meaningful test names
- Include edge cases

### Manual Testing

Test with various scenarios:
- Different container states
- Network connectivity issues
- LXD service interruptions
- Large numbers of containers

## Documentation

### Code Documentation

- Document public APIs with rustdoc comments
- Include examples in documentation
- Keep README.md updated
- Update KEYBINDINGS.md for UI changes

### Commit Messages

Use conventional commit format:
```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat` - New features
- `fix` - Bug fixes  
- `docs` - Documentation updates
- `style` - Code formatting
- `refactor` - Code restructuring
- `test` - Test additions/fixes
- `chore` - Maintenance tasks

Examples:
```
feat(ui): add container creation wizard
fix(lxd): handle connection timeout errors
docs(readme): update installation instructions
```

## Pull Request Process

### Before Submitting

- [ ] Tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy warnings addressed (`cargo clippy`)
- [ ] Documentation updated if needed
- [ ] CHANGELOG.md updated for significant changes

### Pull Request Template

When creating a PR, include:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Refactoring

## Testing
- [ ] Unit tests added/updated
- [ ] Manual testing completed
- [ ] All existing tests pass

## Screenshots (if UI changes)
[Include relevant screenshots]
```

### Review Process

1. Automated checks must pass
2. At least one maintainer review required
3. Address review feedback promptly
4. Maintainer will merge when approved

## Issue Reporting

### Bug Reports

Include:
- LXTUI version
- Operating system
- LXD version
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs (`RUST_LOG=debug lxtui`)

### Feature Requests

Include:
- Clear description of the feature
- Use case and motivation
- Possible implementation approach
- Any relevant mockups or examples

## Architecture Overview

### Key Components

- **app.rs** - Core application state and logic
- **ui.rs** - User interface rendering with ratatui
- **lxd_api.rs** - LXD REST API client
- **main.rs** - Event handling and application lifecycle

### Design Principles

- **Responsive UI** - Operations should not block the interface
- **Error Recovery** - Handle failures gracefully with retry logic
- **User Experience** - Intuitive keyboard shortcuts and clear feedback
- **Performance** - Efficient rendering and minimal resource usage

## Areas for Contribution

### High Priority

- Package manager integrations (apt, yum, homebrew)
- Enhanced error messages and recovery
- Performance optimizations
- Additional container configuration options

### Medium Priority

- Container resource monitoring
- Batch operations on multiple containers
- Import/export functionality  
- Configuration file support

### Low Priority

- Themes and customization
- Plugin system
- Alternative container runtimes
- Advanced filtering and search

## Getting Help

- Check existing issues and discussions
- Join project discussions
- Ask questions in pull requests
- Reach out to maintainers

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help others learn and grow
- Follow the Golden Rule

## Recognition

Contributors will be:
- Listed in project credits
- Mentioned in release notes for significant contributions
- Invited to join the maintainer team for sustained contributions

Thank you for helping make LXTUI better!