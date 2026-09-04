#!/bin/sh
# Install rust-fmt-mf, the standalone macro formatter, so Vim and Neovim find
# it on PATH. The VS Code extension bundles its own copy and needs none of this.
#
#   curl -fsSL https://raw.githubusercontent.com/vremyavnikuda/rust-fmt/main/install.sh | sh
#
# RUSTFMT_MF_VERSION=v0.1.14  pin a release instead of the latest one
# RUSTFMT_MF_BIN_DIR=/some/dir  install somewhere other than ~/.local/bin

set -eu

REPO='vremyavnikuda/rust-fmt'

die() {
    echo "install: $1" >&2
    exit 1
}

detect_os() {
    case $(uname -s) in
        Linux) echo linux ;;
        Darwin) echo darwin ;;
        *) die "unsupported operating system $(uname -s); on Windows run install.ps1 from PowerShell" ;;
    esac
}

detect_arch() {
    case $(uname -m) in
        x86_64 | amd64) echo x64 ;;
        aarch64 | arm64) echo arm64 ;;
        *) die "unsupported architecture $(uname -m)" ;;
    esac
}

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2" || die "cannot download $1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1" || die "cannot download $1"
    else
        die 'neither curl nor wget is available'
    fi
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        die 'neither sha256sum nor shasum is available to verify the download'
    fi
}

# The binary is executed on every format, so a corrupted or substituted
# download is worth one extra request to rule out.
verify() {
    actual=$(sha256_of "$1")
    expected=$(tr -d ' \t\r\n' < "$2")
    [ "$actual" = "$expected" ] || die "checksum mismatch: expected $expected, got $actual"
}

path_hint() {
    case ${SHELL##*/} in
        fish) echo "  fish_add_path $1" ;;
        zsh) echo "  echo 'export PATH=\"$1:\$PATH\"' >> ~/.zshrc" ;;
        *) echo "  echo 'export PATH=\"$1:\$PATH\"' >> ~/.bashrc" ;;
    esac
}

main() {
    os=$(detect_os)
    arch=$(detect_arch)
    asset="rust-fmt-mf-${os}-${arch}"
    version=${RUSTFMT_MF_VERSION:-latest}
    bin_dir=${RUSTFMT_MF_BIN_DIR:-$HOME/.local/bin}

    if [ "$version" = latest ]; then
        base="https://github.com/${REPO}/releases/latest/download"
    else
        base="https://github.com/${REPO}/releases/download/${version}"
    fi

    tmp=$(mktemp -d) || die 'cannot create a temporary directory'
    trap 'rm -rf "$tmp"' EXIT INT TERM

    echo "Downloading ${asset} (${version})"
    fetch "${base}/${asset}" "${tmp}/rust-fmt-mf"
    fetch "${base}/${asset}.sha256" "${tmp}/rust-fmt-mf.sha256"
    verify "${tmp}/rust-fmt-mf" "${tmp}/rust-fmt-mf.sha256"

    mkdir -p "$bin_dir" || die "cannot create $bin_dir"
    chmod +x "${tmp}/rust-fmt-mf"
    mv -f "${tmp}/rust-fmt-mf" "${bin_dir}/rust-fmt-mf" || die "cannot write to $bin_dir"

    echo "Installed ${bin_dir}/rust-fmt-mf"
    "${bin_dir}/rust-fmt-mf" --help >/dev/null 2>&1 || die 'the installed binary does not run'

    case ":${PATH}:" in
        *":${bin_dir}:"*) echo 'Ready: rust-fmt-mf is on your PATH.' ;;
        *)
            echo ''
            echo "${bin_dir} is not on your PATH. Add it:"
            path_hint "$bin_dir"
            ;;
    esac
}

# Called last so a truncated download cannot execute half a script.
main
