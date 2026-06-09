<div align="center">
  <img src="assets/rust_fmt_logo.png" alt="rust fmt logo" width="680">
</div>

# rust fmt

VS Code extension for formatting Rust code with `rustfmt`.

Supports:
- File formatting (including format on save)
- Workspace formatting (`cargo fmt`-optimized)
- Git-aware formatting (`changed` / `staged` Rust files)
- Status bar + Control Center quick actions
- Native `macro_rules!` body formatting (opt-in)

[Marketplace](https://marketplace.visualstudio.com/items?itemName=vremyavnikuda.rust-fmt)

## Requirements

```bash
rustup component add rustfmt
```

Works on **Linux, Windows, macOS**.

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `rustfmt.path` | `"rustfmt"` | Path to rustfmt executable |
| `rustfmt.extraArgs` | `[]` | Additional rustfmt arguments |
| `rustfmt.onboarding.mode` | `"quiet"` | Onboarding mode (`quiet` / `guided`) |
| `nativeMacroFormatter.native` | `false` | Enable `macro_rules!` body formatting |
| `nativeMacroFormatter.path` | auto | Path to `rust-fmt-mf` binary |

Set as default formatter in `settings.json`:

```json
"[rust]": {
    "editor.defaultFormatter": "vremyavnikuda.rust-fmt",
    "editor.formatOnSave": true
}
```

## Commands

| Command | Description |
|---------|-------------|
| Format Document with rustfmt | Current file (on save) |
| Format Workspace with rustfmt | All Rust files in workspace |
| Format Changed Rust Files | Files from `git diff` |
| Format Staged Rust Files | Files from `git diff --cached` |
| Open Control Center | Quick action menu |
| Open Logs | Output channel |

## How it works

Runs `rustfmt --emit stdout` with auto-detected crate root, edition, and config. When `nativeMacroFormatter.native` is enabled, formats `macro_rules!` bodies via `rust-fmt-mf` (skipped by standard rustfmt).

Also detects `rust-toolchain` and `rustfmt.toml`, skips files over 2 MB.

## Troubleshooting

- `rustfmt` not found: run `rustup component add rustfmt` or set `rustfmt.path`.
