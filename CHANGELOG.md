# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.13 - 2026-09-04

### Changed

- Blank lines are preserved by default. Five rules in `normalize_layout_gaps` deleted vertical whitespace that `rustfmt` keeps — inside any braces, between `struct` fields, after an attribute, between repeated invocations of one macro, and between `mod` declarations — so formatting a file for the first time produced a large diff that had nothing to do with macros, on ordinary code that contained none. All five now sit behind one switch, off by default, and the default output matches plain `rustfmt` byte for byte on code without macros. **This changes behavior for existing users:** set `rustfmt.compactBlankLines` to `true` (or pass `--compact-blank-lines` to `rust-fmt-mf`) to keep the previous, more compact output. Only the rules that *remove* vertical space are affected; the blank lines this formatter inserts between items are unchanged.

### Added

- `rustfmt.compactBlankLines` setting and the matching `--compact-blank-lines` flag on `rust-fmt-mf`.

### Known limitations

- Blank lines written inside a `macro_rules!` body are still removed regardless of the setting. That path does not go through `normalize_layout_gaps` at all: the arm-body renderer works on non-blank lines and re-indents them, so preserving them there is a change to how bodies are rendered rather than a rule to switch off.

## 0.1.12 - 2026-09-04

### Changed

- Formatting a macro-heavy file is roughly three times faster, from two `rustfmt` subprocesses out of every three being redundant. The convergence loop re-asks `rustfmt` the same question on every pass, and `format_invocation_inner`/`preformat_rep_bodies` ask it twice per fragment through `try_format_as_mod().or_else(try_format_as_fn())`; since `rustfmt` is deterministic for a given binary, arguments and stdin, both successes and rejections are now served from a process-local memo. On top of that, the `mod` probe is skipped outright when the fragment cannot be a sequence of items — repetition and invocation bodies are usually statements, which never parse inside a `mod`, so that first spawn was always wasted. Measured on Windows against 0.1.11: `real_macro_heavy.rs` (356 lines) goes from 53 spawns and 2517 ms to 17 spawns and 842 ms, `real_macro_edge_cases.rs` (404 lines) from 61 and 2897 ms to 30 and 1450 ms, and a file with no macros at all from 3 spawns to 2. Output is byte-identical on every file checked, and the fixture corpus still reports 80/80 execution safety and 75/75 exact-output conformance.
- Dropped the `syn` dependency, which was declared but never called.

### Fixed

- A file was abandoned to plain `rustfmt`, with a "macro bodies left unformatted" warning, whenever it had two top-level `use` items separated by a blank line — which is to say on essentially every idiomatic Rust file, this repository's own `parser.rs` included. `normalize_layout_gaps` collapsed that blank line, which merges two import groups; `rustfmt` then sorts across the seam, and the token-preservation oracle correctly reported the reordering as changed code. The compaction now applies to `mod` declarations only, the case its test actually covers.
- The same abandonment happened when `rustfmt` removed the braces of a single-expression `match` arm that had come to fit on one line — a rewrite `rustfmt` is entitled to make on ordinary code, which the oracle rejected because it tolerated only added `,`, `{` and `}`, never removed ones. Removals are now accepted on a whole-file pass of plain `rustfmt`, which runs with `format_macro_bodies=false` and so never reaches inside a macro body. Every macro-body rewrite keeps the strict check: losing the braces of `move || { $body }` inside a repetition changes what the macro expands to.
- A `macro_rules!` written as argument text inside another macro's invocation, such as a `quote! { .. }` block, was taken for an item of the file and reformatted. `parse_macro_defs` scanned the whole token stream without regard for enclosing invocation delimiters. On this repository's own `shadow.rs` that flattened a `quote!` block's indentation and emitted `macro_rules !__mf_rep_star`, leaving it further from plain `rustfmt` than it had started. A definition nested inside another definition is unaffected.
- `rust-fmt: Open Logs` showed almost nothing. Thirty-three of the thirty-six log lines went to `console.*`, which lands in the Extension Host devtools console rather than the extension's own output channel, so a failed format left nothing to look at in the place the command opens. Everything now goes through a single `LogOutputChannel`: verbosity is VS Code's own log-level picker instead of a setting of ours, per-format detail is `debug` and notable events are `info`.

## 0.1.11 - 2026-09-04

### Added

- `rust-fmt: Check Formatting` command, also reachable from the Control Center. It formats every file in the chosen scope (workspace, `git diff` working tree, or `git diff --cached`) in memory without writing anything, and reports each file that would change as warnings in the Problems panel — one entry per contiguous block of lines that differ, so the markers point at the actual regions instead of at the file as a whole. Equivalent to `cargo fmt --check`, except that it accounts for `macro_rules!` bodies, which `cargo fmt` leaves alone.
- Vim and Neovim support, with no plugin required: `rust-fmt-mf` is a stdin-to-stdout filter, which is exactly what `formatprg` and conform.nvim expect from an external formatter. README now carries copy-paste configuration for both. `rustfmt.toml` and `rust-toolchain.toml` need no flags — `rustfmt` finds its own config by walking up from the working directory, and the `rustup` shim resolves a pinned toolchain the same way; only the edition has to be named in the editor config, since the formatter reads from stdin and never learns the file's path, and a mismatch fails with a `rustfmt` parse error rather than producing wrong output.
- A release workflow that turns a `v*` tag into a full release with no manual step. Each of the six platforms (`linux-x64`, `linux-arm64`, `win32-x64`, `win32-arm64`, `darwin-x64`, `darwin-arm64`) builds its own binary, trims `bin/` to that platform, and packages a platform-specific `.vsix`; the extension then publishes to the Marketplace only once all six succeeded, and the binaries and packages are attached to a GitHub Release for editors other than VS Code. A per-platform package is roughly 0.5 MB against the 7 MB of the universal one, which carried every platform's binary so that one could be picked at runtime. Two guards run before anything is built: the tag has to match the version in `package.json`, and that version must not already exist on the Marketplace — versions there are immutable, so a re-pushed tag can never republish, and without the check it would fail only after six builds had already overwritten the release assets.
- Visible reporting when the native macro formatter gives up. `formatWithNativeMacroFormatter` now returns a typed outcome (`missing-binary`, `canceled`, `timeout`, `spawn-error`, `exit-code`) instead of a bare `null`, so the extension can tell an ordinary cancellation apart from a real failure. Anything but a cancellation switches the status bar to `rust-fmt: macros skipped` with the reason in its tooltip, writes the reason to the output channel, and shows one notification per reason per session. The fallback to plain `rustfmt` is unchanged — previously it happened silently and a file with unformatted macro bodies looked like a successful format.

### Fixed

- A space before `!=` was deleted inside the arguments of a user-defined macro invocation: `ensure!(source != output, ...)` became `ensure!(source!= output, ...)`. `canonical_token_spacing` treated every `!` as a macro bang and glued it to the preceding token. A `!` is now joined to the name before it only when it is followed by `(`, `[`, or `{`, leaving `!=` and unary `!` alone.
- Generated items inside a macro body were left partly collapsed when the unformatted source put an item's opening brace and its contents on one line, or trailed a closing brace behind other tokens. `pub struct $name<$lt> { pub value: &$lt $ty,` kept its field on the header line, and `fn $method(...) -> $ret { unimplemented!() } }` kept both closing braces there. A pass now breaks any line whose braces do not balance within it, and the existing repair passes fix the indentation from there.
- A `where` clause sitting on its own line kept whatever bounds the unformatted source had left trailing after it. `expand_generated_where_clauses` only recognised a `where` that shared a line with its `struct`, `enum`, or `union`.
- Together these three restore the `test-rs` corpus exactly — 1800 lines, nothing left unformatted — from a scrambled state, verified across several scrambling seeds.

## 0.1.10 - 2026-08-23

### Fixed

- Formatting a file with CRLF line endings and at least one `//` comment failed closed (`SKIPPED` or a hard error) instead of formatting, because `rustfmt` and this crate's own shadow-file processing silently drop `\r` on output while the safety oracle correctly detects the resulting change to a comment's literal text. `format_source`/`format_source_with_report` now detect CRLF input, normalize to `\n` for every internal pass, and restore `\r\n` (remapping reported macro spans accordingly) in the final output. LF-only input is unaffected.
- Added `.gitattributes` forcing LF line endings for all tracked text files, so checking out this repository on a Windows machine with the common `core.autocrlf=true` default no longer turns committed `.rs` fixtures into CRLF in the first place.

## 0.1.9 - 2026-08-23

### Changed

- Macro definitions in the same file are now formatted in one combined `rustfmt` call per convergence pass instead of one call per definition, falling back to the previous per-definition behavior only if the batch fails the token-preservation check. Measured on a 21-macro fixture using `rustfmt_call_count()` (successful `Command::spawn()` calls in-process, not `execve` events — a naive `strace -f -e trace=execve` count double-counts every call because each `rustfmt` invocation on this rustup setup execs twice, through the `~/.cargo/bin/rustfmt` shim and then the real toolchain binary): `rustfmt` subprocess spawns dropped from 47 to 27, wall-clock time from ~1.0s to ~0.62s.

## 0.1.8 - 2026-08-22

### Added

- Explicit per-macro formatting outcomes: `FORMATTED`, `UNCHANGED`, and `SKIPPED` with a reason and source range. Diagnostics are written to stderr while stdout remains valid formatted Rust.
- A safety oracle that verifies exact significant-token preservation, complete-file Rust syntax, and byte-identical output on a second formatting pass.
- Automatic discovery and auditing of every golden fixture and every Rust source file under `test-rs/src`, including exact comparison of the four real macro corpus files with their user-approved outputs.
- Adversarial regression fixtures for matcher line comments, literal delimiters, Unicode identifiers, synthetic-marker collisions, `macro_rules!` text inside literals and comments, all definition/transcriber delimiters, arbitrary repetition separators, tuple trailing commas, and opaque macro DSLs.
- Compilation validation of a temporary fully formatted `test-rs` copy with `cargo check --all-targets`.
- Native binary path and SHA-256 logging in the VS Code extension for detecting stale bundled artifacts.
- Native binary support for `linux-arm64` and `win32-arm64`, extending bundled platform coverage from four targets to six.
- CI validation of the native formatter on `macos-latest`, `macos-13`, and `ubuntu-24.04-arm` runners, in addition to the existing Linux x64 and Windows x64 coverage.

### Changed

- Replaced byte-level `macro_rules!` discovery with `ra-ap-rustc_lexer`, exact UTF-8 byte ranges, and typed matching for `()`, `[]`, and `{}` delimiters.
- Native formatting now processes macro definitions independently. Unsupported or lossy transformations preserve the original macro and report `SKIPPED` instead of returning partially rewritten code.
- Macro matchers, transcribers, generated `macro_rules!` definitions, and custom macro invocations now use token-aware spacing and delimiter handling instead of raw string replacements.
- Parenthesized and bracketed transcribers, arbitrary repetition separators, nested repetitions, item/block invocations, and comment-bearing matchers are formatted losslessly.
- Synthetic metavariable, repetition, shadow, and final-pass markers now use a collision-free prefix verified to be absent from the source.
- Formatting now converges to a fixed point in at most eight token-preserving passes. Non-converging input still fails closed instead of returning unstable output.
- The VS Code extension now sends the original document directly to the native formatter. Native failure falls back to ordinary `rustfmt` using the same original text, without TypeScript spacing or indentation rewrites.
- The current-platform build verifies that the release artifact and copied `bin/<platform>-<arch>` binary have identical SHA-256 hashes.
- Rebuilt every bundled native formatter binary (`linux-x64`, `linux-arm64`, `win32-x64`, `win32-arm64`, `darwin-x64`, `darwin-arm64`) against the new safety pipeline.
- Generalized the CI executable-bit check to every non-Windows target instead of only `linux-x64`.
- Renamed the audit summary to distinguish execution safety, exact-output conformance, non-skipped macro handling, and macros actually changed on the current input. `UNCHANGED` is no longer presented as a successful formatting change.
- Nested Rust blocks now use a deterministic compact layout: arbitrary blank lines between statements are removed while top-level items remain separated.
- Macro repetitions containing statements are expanded to a block layout with structural indentation; expression-only repetitions remain compact.
- Native formatting now runs for every Rust document, so ordinary functions, structs, `impl` blocks, and module layout use the same verified pipeline even when the file has no `macro_rules!` definition.
- Corpus files without an approved macro golden must now match an independent rustfmt pass exactly; syntax-only success can no longer be reported as correct formatting.

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
- Incomplete fixture coverage caused by a manually maintained fixture list.
- False-positive corpus results where `test-rs` was checked only for successful execution, syntax, and idempotence but was not compared with the approved output. Corpus golden differences now fail the audit and include an exact diff artifact.
- Ambiguous golden failures: differences limited to added or removed blank lines are now reported as `GOLDEN_BLANK_LINES`, separately from token spacing, indentation, wrapping, or content differences reported as `GOLDEN_DIFF`.
- Broken audit unit tests that referenced a missing `parse_args` function.
- Flat `$()` statement repetitions such as `$(let value = expression;)?`, which were previously left on one line and made macro bodies visually inconsistent.
- Compact generated Rust items now expand structurally inside macro transcribers, including `enum`, `struct ... where`, nested `impl`/`fn` bodies, and optional repetitions of named fields.
- Generated `where` clauses no longer depend on the input line breaks, and const-expression braces inside their predicates are not mistaken for the item body.
- Matchers containing `//` comments now place their closing delimiter on a separate correctly indented line, preventing visually attached or swallowed matcher tokens.
- Missing trailing commas in multiline structs generated by macros, including `$item:item` and `$ty:ty` bodies.
- Incorrect indentation of nested generated `macro_rules!` definitions such as `make_tripler!`.
- Partially expanded generated structs and `impl` blocks whose closing brace or nested function remained on the wrong line.
- Long macro matchers and invocations now wrap deterministically, including the trailing semicolon in width calculations.
- Random blank lines inside fields, parameters, `where` clauses, and statements are removed while module items and methods retain one structural separator.
- A stray `max_width=80` / `chain_width=40` override in the non-macro rustfmt pass made every formatted file — macro or not — wrap significantly more aggressively than `cargo fmt`'s real defaults (`max_width=100`, `chain_width=60`). Removed the override so output matches an unconfigured `rustfmt` exactly; workspaces with their own `rustfmt.toml` were never affected.
- Invocations of user-defined macros with a long comma-separated argument list (e.g. `my_macro!(1, 2, 3, ...)`) were exploded to one item per line instead of packing greedily like rustfmt does for `vec!`. `format_dsl_comma_list` now fills each line up to the style width before wrapping, matching `rustfmt`'s own line-filling behavior.
- `test-rs/src/examples/macro_edge_cases.rs` had accumulated blank lines between doc-comment bullets and macro definitions that its scrambled sibling fixture (`tests/fixtures/real_macro_edge_cases.rs`) never had; since `rustfmt` treats blank lines between comments as significant and does not collapse them, the two inputs could never converge to the same golden. Removed the stray blank lines so both sources format identically.
- `Format Workspace` only re-ran the native macro formatter on files whose text literally contained `macro_rules!`, so files without a macro definition kept whatever plain `cargo fmt` produced instead of the native pipeline's output — including redundant blank lines the native pass would otherwise collapse. The native pass now runs on every file after the bulk `cargo fmt` warm-up, matching the result of formatting a single file on save.

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

---

## 0.1.2 - 2026-01-28

### Added

- Temporary workspace formatting cache for resolved Rust context (crate root, config, toolchain) to reduce repeated filesystem lookups.

---

## 0.1.1 - 2026-01-26

### Added

- Workspace formatting command `rust-fmt.formatWorkspace` and `Shift+Alt+F`/`Shift+Option+F` binding for Rust files.
- Status bar indicator ("rust-fmt: active") with quick access to workspace formatting.
- Cancellation support and protection against parallel formatting runs per file.
- File size guard (skip formatting files larger than 2 MB).
- Auto-detect `Cargo.toml` to set crate root and `--edition`.
- Auto-detect `rustfmt.toml` / `.rustfmt.toml` and pass `--config-path`.
- Auto-detect `rust-toolchain(.toml)` and set `RUSTUP_TOOLCHAIN` when running `rustfmt`.
