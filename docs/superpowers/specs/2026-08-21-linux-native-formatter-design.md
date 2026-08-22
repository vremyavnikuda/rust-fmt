# Linux Native Macro Formatter Design

## Goal

Make the bundled `rust-fmt-mf` formatter work reliably on Linux x86_64 without changing the existing Windows execution path.

## Scope

- Fix current-platform detection in the native build script.
- Preserve executable permissions when producing Unix binaries.
- Rebuild the bundled Linux x86_64 binary from the current source.
- Run the existing macro corpus against an explicitly selected binary.
- Detect crashes, invalid output, golden-output drift, and non-idempotent formatting.
- Test bundled Linux and Windows binaries on their native CI runners.
- Preserve the extension's safe fallback to ordinary `rustfmt` when native execution fails.

Linux ARM64, runtime downloads, and install-time Rust builds are out of scope.

## Architecture

`rust-fmt-mf` remains the source of formatter behavior. `scripts/build_current.py` builds the current native target and copies it to `bin/<platform>/`, setting executable permissions only on Unix.

The existing Python fixture runner gains a `--binary` option and becomes the single native-binary acceptance check. CI builds the native formatter, runs Rust tests, and then runs the acceptance check against the exact bundled path used by the extension.

## Validation Contract

For every fixture, the acceptance check requires:

1. The formatter exits successfully.
2. Output matches the checked-in `.expected` file.
3. Plain `rustfmt` accepts the produced Rust.
4. Formatting the result again produces identical bytes.

On failure, the runner prints a readable diff and stores input, output, stderr, and second-pass output under `target/macro-audit/<fixture>/`.

## CI

The existing TypeScript job remains unchanged. A native matrix adds Ubuntu x86_64 and Windows x86_64 jobs. Each job:

1. installs stable Rust with `rustfmt`;
2. runs `cargo test --all-targets`;
3. builds and copies the current native binary;
4. runs the fixture acceptance check against the bundled binary;
5. uploads `target/macro-audit` only when a failure occurs.

Linux additionally asserts that `bin/linux-x64/rust-fmt-mf` is executable. Windows continues to use `bin/win32-x64/rust-fmt-mf.exe` and receives no Unix permission handling.

## Error Handling

The extension keeps the existing behavior: a missing, non-executable, timed-out, or failed native process returns no native result and falls back to ordinary `rustfmt`. The native error includes the selected path or exit status in the extension-host log.

## Testing

- Unit-test platform/architecture mapping in the build script using standard-library `unittest`.
- Exercise all existing fixtures through both first and second formatter passes.
- Run the bundled binary on native Linux and Windows CI runners.
- Keep the repository clean after generated diagnostic artifacts by placing them under ignored Cargo `target/`.
