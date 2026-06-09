# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.6 - 2026-06-10

### Added
- Full cross-platform support: native macro formatter binaries for Linux (`linux-x64`) and macOS (`darwin-x64`, `darwin-arm64`) in addition to Windows.

## 0.1.5 - 2026-06-09

### Added
- Native macro formatter (`rust-fmt-mf`) for formatting `macro_rules!` bodies.
- New `nativeMacroFormatter.native` and `nativeMacroFormatter.path` settings to enable and configure native macro formatting.

### Fixed
- Incorrect body indentation in `struct_with_bounds!` macros.
- Extra spaces before colon in `$()` repetition patterns.

## 0.1.4 - 2026-05-31

### Changed
- Updated workspace format shortcut to `Ctrl+Alt+Shift+F` / `Cmd+Option+Shift+F` and clarified command naming.
- Parallel filesystem searches in context resolution for faster formatting.
- Context cache with mtime-based invalidation reduces repeated filesystem lookups during format-on-save.

### Added
- New Git-based formatting commands: `Format Changed Rust Files` and `Format Staged Rust Files`.
- New Control Center and Logs commands accessible via Command Palette.
- New `rustfmt.onboarding.mode` setting (`quiet` / `guided`) for default formatter prompts.
- Status bar shows format duration after each format (with loading indicator).
- Format Selection support: format a selected range of lines with `rustfmt --file-lines`.

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
