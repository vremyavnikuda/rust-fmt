![rust-fmt banner](assets/baner.png)

# rust fmt

Simple VS Code extension for formatting Rust code using `rustfmt`.

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

## Usage

**Automatic:** Save any `.rs` file (if `formatOnSave` is enabled).

**Manual:**
- Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`) -> "Format Document with rustfmt" (current file)
- Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`) -> "Format Workspace with rustfmt" (all Rust files; uses `cargo fmt` when all files are saved)
- Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`) -> "Use rust-fmt as Default Formatter" (prompts for Global or Workspace, then enables format on save)
- Shortcut: `Shift+Alt+F` (Windows/Linux) or `Shift+Option+F` (Mac) formats the entire workspace when a Rust file is active
- Status bar: "rust-fmt: active" appears for Rust files; click to format workspace

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

