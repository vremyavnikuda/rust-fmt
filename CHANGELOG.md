# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added
- Explicit per-macro formatting outcomes: `FORMATTED`, `UNCHANGED`, and `SKIPPED` with a reason and source range. Diagnostics are written to stderr while stdout remains valid formatted Rust.
- A safety oracle that verifies exact significant-token preservation, complete-file Rust syntax, and byte-identical output on a second formatting pass.
- Automatic discovery and auditing of every golden fixture and every Rust source file under `test-rs/src`, with separate safety, golden-output, and deep-format coverage metrics.
- Adversarial regression fixtures for matcher line comments, literal delimiters, Unicode identifiers, synthetic-marker collisions, `macro_rules!` text inside literals and comments, all definition/transcriber delimiters, arbitrary repetition separators, tuple trailing commas, and opaque macro DSLs.
- Compilation validation of a temporary fully formatted `test-rs` copy with `cargo check --all-targets`.
- Native binary path and SHA-256 logging in the VS Code extension for detecting stale bundled artifacts.

### Changed
- Replaced byte-level `macro_rules!` discovery with `ra-ap-rustc_lexer`, exact UTF-8 byte ranges, and typed matching for `()`, `[]`, and `{}` delimiters.
- Native formatting now processes macro definitions independently. Unsupported or lossy transformations preserve the original macro and report `SKIPPED` instead of returning partially rewritten code.
- Macro matchers, transcribers, generated `macro_rules!` definitions, and custom macro invocations now use token-aware spacing and delimiter handling instead of raw string replacements.
- Parenthesized and bracketed transcribers, arbitrary repetition separators, nested repetitions, item/block invocations, and comment-bearing matchers are formatted losslessly.
- Synthetic metavariable, repetition, shadow, and final-pass markers now use a collision-free prefix verified to be absent from the source.
- Removed the arbitrary four-pass convergence loop; one formatting pass must now be idempotent or fail closed.
- The VS Code extension now sends the original document directly to the native formatter. Native failure falls back to ordinary `rustfmt` using the same original text, without TypeScript spacing or indentation rewrites.
- The current-platform build verifies that the release artifact and copied `bin/<platform>-<arch>` binary have identical SHA-256 hashes.
- Rebuilt the bundled Linux x64 native formatter with the new safety pipeline while leaving other platform artifacts unchanged.

### Fixed
- False detection of `macro_rules!` inside strings and comments, and premature delimiter closure caused by character literals or comments containing braces and parentheses.
- UTF-8 corruption when formatting Unicode identifiers and comments.
- Matcher corruption when collapsing a newline after a `//` comment.
- Changes to string literal contents caused by global spacing replacements.
- Collisions with user identifiers and comments resembling internal names such as `__m_0`, `__mf_rep_*`, and `__mf_nm_0__`.
- Loss of semantically significant tuple trailing commas.
- Loss of nested block braces when extracting formatted shadow macro bodies.
- Incorrect indentation and second-pass drift in nested blocks and repetition bodies.
- Unsafe removal of closure blocks inside macro repetitions.
- Incorrect whitespace around compound operators, postfix `?`, paths, generic arguments, macro fragment specifiers, and `$()` repetition operators.
- First-pass indentation drift caused by using the original whitespace before a top-level `macro_rules!` definition as its structural nesting level.
- Incomplete fixture coverage caused by a manually maintained fixture list. The current release audit is 100% safety coverage (80/80 files), 100% golden coverage (71/71 fixtures), and 100% deep-format coverage (200/200 macros), including all nine Rust files under `test-rs/src` and compilation of their formatted copy.

## 0.1.7 - 2026-06-22

### Fixed
- Raw string closing delimiter order in parser: was `#"` (hash-then-quote) instead of `"#` (quote-then-hash), causing unterminated raw strings that swallowed subsequent macro definitions.
- Off-by-one in parser brace matching when a string escape `\\` is at the last byte of a macro body.
- Empty arm bodies now parse correctly (zero-length body extraction).

### Changed
- Rewrote `normalize_body_indent` — replaced heuristic min_indent/has_closer approach with a state machine that tracks structural depth (`{`/`}`, `$(`/`)+` repetition, `where` clauses). Normalization now runs before `$()` replacement so the original macro syntax is visible to the depth tracker. Fixes indentation for macros with where clauses, nested repetitions, inline braces, and multi-level `$()` nesting.
- Rewrote shadow file builder (`build_shadow_file_from_strings`) to preserve relative indentation via min-indent stripping instead of adding a uniform 4-space indent.
- Added single-line arm body extraction in `split_shadow_into_arms` for arms with `() => { BODY };` on one line.
- Added `macro_end.min(source.len())` safety clamp in `scan_arms`.
- Pre-computed brace counts per line in VS Code extension `normalizeMacroBodies` — replaces O(n²) `countChar` calls with O(1) array lookups; moved `normalizeMacroSpacing` before native formatter path.

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
