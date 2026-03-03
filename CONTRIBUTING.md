# Contributing

Contributions are welcome! Here's how to get started.

## Development Setup

```bash
git clone https://github.com/OpenScribbler/phyllotaxis.git
cd phyllotaxis
cargo build
cargo test
```

## Before Submitting a PR

All four CI gates must pass:

```bash
cargo test --locked
cargo clippy --locked -- -D warnings
cargo fmt --check
cargo deny check
```

## Reporting Bugs

Open an issue on [GitHub Issues](https://github.com/OpenScribbler/phyllotaxis/issues) with:
- The command you ran
- What you expected
- What happened instead
- Your spec file (or a minimal reproduction) if possible

## Feature Requests

Open an issue describing the use case. Explaining *why* you need something helps more than describing *what* to build.
