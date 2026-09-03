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
- Native `macro_rules!` body formatting
- Standalone binary usable from Vim / Neovim without a plugin

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
| `macroFormatter.native` | `true` | Enable `macro_rules!` body formatting |
| `macroFormatter.path` | auto | Path to `rust-fmt-mf` binary |

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
| Check Formatting | Report unformatted files without changing them |
| Open Control Center | Quick action menu |
| Open Logs | Output channel |

## Vim / Neovim

The macro formatter is a plain stdin-to-stdout filter, so no plugin is needed.

### Install

Download `rust-fmt-mf` for your platform from the
[latest release](https://github.com/vremyavnikuda/rust-fmt/releases/latest),
put it on your `PATH`, and (outside Windows) make it executable:

```bash
chmod +x rust-fmt-mf
```

Or build it yourself:

```bash
cargo install --git https://github.com/vremyavnikuda/rust-fmt rust-fmt-mf
```

### Vim

```vim
autocmd FileType rust setlocal formatprg=rust-fmt-mf\ --edition\ 2021
```

Format the buffer with `gggqG`, or a selection with `gq`.

### Neovim with conform.nvim

Preferred over `formatprg`: it keeps the cursor position and marks, and gives
you format-on-save.

```lua
require("conform").setup({
  formatters = {
    ["rust-fmt-mf"] = {
      command = "rust-fmt-mf",
      args = { "--edition", "2021" },
      stdin = true,
    },
  },
  formatters_by_ft = { rust = { "rust-fmt-mf" } },
  format_on_save = { timeout_ms = 2000, lsp_format = "never" },
})
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--edition` | `2021` | Rust edition passed to `rustfmt` |
| `--rustfmt-path` | `rustfmt` | Path to the `rustfmt` executable |
| `--config-path` | auto | Path to `rustfmt.toml` / `.rustfmt.toml` |

`rustfmt.toml` and `rust-toolchain.toml` are picked up from the working
directory upwards without any flags — `rustfmt` searches for its own config,
and the `rustup` shim resolves the pinned toolchain the same way. Keep your
editor's working directory inside the project and both just work.

The edition is the exception: it has to be in your editor config, because the
formatter reads from stdin and never learns the file's path. A crate on a
different edition fails loudly with a `rustfmt` parse error rather than
producing wrong output — change the `--edition` value to match.

## How it works

Runs `rustfmt --emit stdout` with auto-detected crate root, edition, and config. When `macroFormatter.native` is enabled, formats `macro_rules!` bodies via `rust-fmt-mf` (skipped by standard rustfmt).

Also detects `rust-toolchain` and `rustfmt.toml`, skips files over 2 MB.

## Troubleshooting

- `rustfmt` not found: run `rustup component add rustfmt` or set `rustfmt.path`.
