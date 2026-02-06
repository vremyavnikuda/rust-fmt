# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.3 - 2026-02-06

### Added
- Quick command to set rust-fmt as the default formatter, with Global or Workspace scope selection.
- Smart prompt when Rust is not using rust-fmt as the default formatter.

### Changed
- Workspace formatting is now faster on large projects.

-----
## 0.1.2 - 2026-01-28

### Added
- Temporary workspace formatting cache for resolved Rust context (crate root, config, toolchain) to reduce repeated filesystem lookups.

-----
## 0.1.1 - 2026-01-26

### Added
- Workspace formatting command `rust-fmt.formatWorkspace` and `Shift+Alt+F`/`Shift+Option+F` binding for Rust files.
- Status bar indicator ("rust-fmt: active") with quick access to workspace formatting.
- Cancellation support and protection against parallel formatting runs per file.
- File size guard (skip formatting files larger than 2 MB).
- Auto-detect `Cargo.toml` to set crate root and `--edition`.
- Auto-detect `rustfmt.toml` / `.rustfmt.toml` and pass `--config-path`.
- Auto-detect `rust-toolchain(.toml)` and set `RUSTUP_TOOLCHAIN` when running `rustfmt`.
