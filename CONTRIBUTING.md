# Contributing to crdt-kit

Thank you for your interest in contributing to crdt-kit! This document provides
guidelines and information for contributors.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/abdielLopezpy/crdt-kit.git`
3. Create a branch: `git checkout -b my-feature`
4. Make your changes
5. Run tests: `cargo test --all-features`
6. Submit a pull request

## Development Setup

```bash
# Clone the repo
git clone https://github.com/abdielLopezpy/crdt-kit.git
cd crdt-kit

# Run tests
cargo test --all-features

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Run formatter
cargo fmt --all -- --check

# Run benchmarks
cargo bench
```

## Pull Request Guidelines

- Keep PRs focused on a single change
- Include tests for new functionality
- Update documentation as needed
- Follow the existing code style
- Add a changelog entry if applicable
- Ensure CI passes before requesting review

## Code Style

- Follow standard Rust conventions (`rustfmt` defaults)
- Use meaningful variable and function names
- Add doc comments for public APIs
- Keep functions focused and small
- Prefer `#[must_use]` on functions returning values that shouldn't be ignored

## Testing

- All new features must include tests
- Use property-based testing where applicable
- Ensure convergence properties hold for all CRDT types
- Run the full test suite before submitting: `cargo test --all-features`

## Commit Messages

Use clear, descriptive commit messages:

```
feat: add delta encoding for GCounter
fix: correct merge behavior for empty MVRegister
docs: update README with benchmark results
test: add property tests for ORSet convergence
```

## Reporting Bugs

Use the GitHub issue tracker with the bug report template. Include:

- A clear description of the bug
- Steps to reproduce
- Expected vs actual behavior
- Rust version and OS

## Feature Requests

Use the GitHub issue tracker with the feature request template. Include:

- Clear description of the feature
- Use case / motivation
- Proposed API (if applicable)

## License

By contributing to crdt-kit, you agree that your contributions will be licensed
under the MIT OR Apache-2.0 license.
