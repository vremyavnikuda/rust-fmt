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
| `rustfmt.extraArgs` | `[]` | Additional rustfmt arguments, also passed on by the macro formatter |
| `rustfmt.onboarding.mode` | `"quiet"` | Onboarding mode (`quiet` / `guided`) |
| `rustfmt.compactBlankLines` | `false` | Delete blank lines inside braces to keep large files compact |
| `rustfmt.formatMacroBodies` | `false` | Pass `format_macro_bodies=true` to rustfmt (nightly toolchain) |
| `rustfmt.formatMacroMatchers` | `false` | Pass `format_macro_matchers=true` to rustfmt (nightly toolchain) |
| `macroFormatter.native` | `true` | Enable `macro_rules!` body formatting |
| `macroFormatter.path` | auto | Path to `rust-fmt-mf` binary |

Set as default formatter in `settings.json`:

```json
"[rust]": {
    "editor.defaultFormatter": "vremyavnikuda.rust-fmt",
    "editor.formatOnSave": true
}
```

Delete blank lines inside braces:

```json
"rustfmt.compactBlankLines": true
```

## Commands

| Command | Description |
|---------|-------------|
| Format Document with rustfmt | Current file (on save) |
| Format Selection with rustfmt | Only the changes that touch the selected lines |
| Format Workspace with rustfmt | All Rust files in workspace |
| Format Changed Rust Files | Files from `git diff` |
| Format Staged Rust Files | Files from `git diff --cached` |
| Check Formatting | Report unformatted files without changing them |
| Open Control Center | Quick action menu (also as Open Behavior Controls) |
| Set as Default Formatter | Make rust-fmt the Rust formatter, globally or for the workspace |
| Open Logs | Output channel |

## Vim / Neovim

The macro formatter is a plain stdin-to-stdout filter, so no plugin is needed.

### Install

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/vremyavnikuda/rust-fmt/main/install.py | python3 -
```

Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/vremyavnikuda/rust-fmt/main/install.py | python -
```

One script for all three systems. It needs Python 3.8 or newer, which most
Linux installs already have and which macOS and Windows get from
[python.org](https://www.python.org/downloads/) or their package manager.

The binary goes in `~/.local/bin` and its SHA-256 is checked against the one
published with the release. On Linux and macOS the script prints the line to
add to your shell config; on Windows it adds the directory to your user `PATH`
itself, so open a new terminal afterwards. `RUSTFMT_MF_VERSION` pins a release
instead of the latest, `RUSTFMT_MF_BIN_DIR` installs somewhere else.

<details>
<summary>Without the script</summary>

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

</details>

### Vim

```vim
autocmd FileType rust setlocal formatprg=rust-fmt-mf\ --edition\ 2021
```

Format the buffer with `gggqG`, or a selection with `gq`. A selection has to
hold whole items, such as complete functions; part of one is left unchanged,
because the formatter cannot parse it on its own. If formatting fails for any
reason, the lines come back exactly as they were.

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
| `--compact-blank-lines` | off | Delete blank lines inside braces |
| `--rustfmt-arg` | none | One extra argument for every `rustfmt` call; repeat it, e.g. `--rustfmt-arg=--config --rustfmt-arg=max_width=80` |

`rustfmt.toml` and `rust-toolchain.toml` are picked up from the working
directory upwards without any flags — `rustfmt` searches for its own config,
and the `rustup` shim resolves the pinned toolchain the same way. Keep your
editor's working directory inside the project and both just work.

The edition is the exception: it has to be in your editor config, because the
formatter reads from stdin and never learns the file's path. A crate on a
different edition fails loudly with a `rustfmt` parse error rather than
producing wrong output — change the `--edition` value to match.

## How it works

Runs `rustfmt --emit stdout` with auto-detected crate root, edition, and config. The edition comes from `[package]` in the nearest `Cargo.toml`, following `edition.workspace = true` to the workspace root. When `macroFormatter.native` is enabled, formats `macro_rules!` bodies via `rust-fmt-mf` (skipped by standard rustfmt).

Also detects `rust-toolchain` and `rustfmt.toml`, skips files over 2 MB.

## Troubleshooting

- `rustfmt` not found: run `rustup component add rustfmt` or set `rustfmt.path`.
- Format Selection changes nothing: the selected lines cannot be formatted apart from the rest of the file, usually because `rustfmt` would move an import elsewhere. The log says so; format the whole document instead.
