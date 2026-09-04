# Install rust-fmt-mf, the standalone macro formatter, so Vim and Neovim find
# it on PATH. The VS Code extension bundles its own copy and needs none of this.
#
#   irm https://raw.githubusercontent.com/vremyavnikuda/rust-fmt/main/install.ps1 | iex
#
# $env:RUSTFMT_MF_VERSION = 'v0.1.14'   pin a release instead of the latest one
# $env:RUSTFMT_MF_BIN_DIR = 'C:\some\dir'  install somewhere other than ~\.local\bin

$ErrorActionPreference = 'Stop'

function Install-RustFmtMf {
    $repo = 'vremyavnikuda/rust-fmt'

    $arch = switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { 'x64' }
        'ARM64' { 'arm64' }
        default { throw "unsupported architecture $($env:PROCESSOR_ARCHITECTURE)" }
    }
    $asset = "rust-fmt-mf-win32-$arch.exe"

    $version = if ($env:RUSTFMT_MF_VERSION) { $env:RUSTFMT_MF_VERSION } else { 'latest' }
    $base = if ($version -eq 'latest') {
        "https://github.com/$repo/releases/latest/download"
    } else {
        "https://github.com/$repo/releases/download/$version"
    }

    # The same path as on Linux and macOS, so the Vim configuration in the
    # README is one text for all three systems.
    $binDir = if ($env:RUSTFMT_MF_BIN_DIR) { $env:RUSTFMT_MF_BIN_DIR } else { Join-Path $HOME '.local\bin' }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $exe = Join-Path $tmp 'rust-fmt-mf.exe'
        Write-Host "Downloading $asset ($version)"
        Invoke-WebRequest -Uri "$base/$asset" -OutFile $exe -UseBasicParsing
        # To a file, not .Content: release assets are served as
        # application/octet-stream, and then .Content is a Byte[] with no Trim.
        $sums = Join-Path $tmp 'rust-fmt-mf.sha256'
        Invoke-WebRequest -Uri "$base/$asset.sha256" -OutFile $sums -UseBasicParsing
        $expected = (Get-Content -Path $sums -Raw).Trim()

        # The binary is executed on every format, so a corrupted or substituted
        # download is worth one extra request to rule out.
        $actual = (Get-FileHash -Path $exe -Algorithm SHA256).Hash.ToLower()
        if ($actual -ne $expected.ToLower()) {
            throw "checksum mismatch: expected $expected, got $actual"
        }

        New-Item -ItemType Directory -Path $binDir -Force | Out-Null
        $target = Join-Path $binDir 'rust-fmt-mf.exe'
        Move-Item -Path $exe -Destination $target -Force
        Write-Host "Installed $target"

        & $target --help | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'the installed binary does not run' }
    } finally {
        Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @()
    if ($userPath) { $entries = $userPath -split ';' | Where-Object { $_ } }
    if ($entries -notcontains $binDir) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $binDir) -join ';'), 'User')
        Write-Host ''
        Write-Host "Added $binDir to your PATH. Open a new terminal for it to take effect."
    } else {
        Write-Host 'Ready: rust-fmt-mf is on your PATH.'
    }
}

# Called last so a truncated download cannot execute half a script.
Install-RustFmtMf
