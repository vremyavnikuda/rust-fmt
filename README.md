![rust-fmt banner](assets/baner.png)

# rust fmt

Simple VS Code extension for formatting Rust code with `rustfmt`.

It supports:
- File formatting (including format on save)
- Workspace formatting (with `cargo fmt` optimization)
- Git-aware formatting (`changed` / `staged` Rust files)
- Quick actions from status bar + Control Center

[Marketplace](https://marketplace.visualstudio.com/items?itemName=vremyavnikuda.rust-fmt)

## Requirements

You need `rustfmt` installed. Install it via:

```bash
rustup component add rustfmt
```

Verify installation:

```bash
rustfmt --version
rustc --version
rustup --version
```

Works on **Linux, Windows, and macOS**. If your project uses `rust-toolchain` files, `rustup` is required so the extension can pick the correct toolchain.
For faster workspace formatting, `cargo` must be available (usually installed with Rust/rustup).

## VS Code Settings

Add to your `settings.json` and set rust-fmt as the default formatter for Rust:

```json
"[rust]": {
    "editor.defaultFormatter": "vremyavnikuda.rust-fmt",
    "editor.formatOnSave": true
}
```

## Extension Settings

- `rustfmt.path`: Path to rustfmt executable (default: "rustfmt")
- `rustfmt.extraArgs`: Additional arguments for rustfmt (default: [])
- `rustfmt.onboarding.mode`: Onboarding prompt mode (`quiet` or `guided`, default: `quiet`)

## Usage

Automatic:
- Save any `.rs` file (when `editor.formatOnSave` is enabled). Rust formatting is applied to the whole file.

Manual commands (Command Palette):
- `Format Document with rustfmt` - format current Rust file
- `Format Workspace with rustfmt` - format all Rust files in workspace
- `Format Changed Rust Files` - format Rust files from `git diff`
- `Format Staged Rust Files` - format Rust files from `git diff --cached`
- `rust-fmt: Open Control Center` - open quick action menu
- `rust-fmt: Open Behavior Controls` - alias to the same Control Center
- `rust-fmt: Set as Default Formatter` - set rust-fmt as default formatter for Rust
- `rust-fmt: Open Logs` - open rust-fmt output channel

Shortcut:
- `Ctrl+Alt+Shift+F` (Windows/Linux) or `Cmd+Option+Shift+F` (Mac) formats the workspace when a Rust file is active.

Status bar:
- `rust-fmt` item appears for Rust files.
- Click opens Control Center.
- Hover shows quick action links (format workspace/changed/staged, logs, reload, etc).

## How it works

The extension runs `rustfmt --emit stdout` on your code and applies the formatted result.

It also:
- Sets the working directory to the nearest `Cargo.toml` (crate root).
- Reads `edition` from `Cargo.toml` and passes `--edition`.
- Finds `rustfmt.toml` / `.rustfmt.toml` and passes `--config-path`.
- If `rust-toolchain(.toml)` is present, sets `RUSTUP_TOOLCHAIN` automatically.
- Skips files larger than 2 MB.
- Uses `cargo fmt` per crate when formatting the workspace and all Rust files are saved; otherwise it falls back to per-file formatting and skips dirty files.

## Troubleshooting

- `rustfmt` not found: install it via `rustup component add rustfmt` or set `rustfmt.path` to the executable.
