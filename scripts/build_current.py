#!/usr/bin/env python3
"""Build rust-fmt-mf for the current platform and copy to bin/.

Usage:
    python scripts/build_current.py               # release build
    python scripts/build_current.py --debug        # debug build
    python scripts/build_current.py --release      # explicit release build
"""

import argparse
import hashlib
import platform
import stat
import subprocess
import sys
from pathlib import Path


def get_project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def get_binary_name() -> str:
    return "rust-fmt-mf.exe" if sys.platform == "win32" else "rust-fmt-mf"


def get_platform(sys_platform: str | None = None, machine: str | None = None) -> str:
    sys_platform = sys_platform or sys.platform
    machine = machine or platform.machine()
    arch = {
        "AMD64": "x64",
        "x86_64": "x64",
        "arm64": "arm64",
        "aarch64": "arm64",
    }.get(machine)
    if arch is None:
        raise ValueError(f"Unsupported architecture: {machine}")
    os_name = (
        "win32"
        if sys_platform == "win32"
        else "darwin"
        if sys_platform == "darwin"
        else "linux"
    )
    return f"{os_name}-{arch}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as binary:
        for chunk in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description="Build rust-fmt-mf for current platform")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--release", action="store_true", help="Release build (default)")
    group.add_argument("--debug", action="store_true", help="Debug build")
    args = parser.parse_args()
    is_release = not args.debug
    build_type = "release" if is_release else "debug"
    profile_flag = "--release" if is_release else ""
    root = get_project_root()
    project_dir = root / "rust-fmt-mf"
    print(f"Building rust-fmt-mf ({build_type})...")
    cmd = ["cargo", "build", "-p", "rust-fmt-mf", "--manifest-path", str(project_dir / "Cargo.toml")]
    if profile_flag:
        cmd.append(profile_flag)
    result = subprocess.run(cmd)
    if result.returncode != 0:
        print("Build failed", file=sys.stderr)
        return 1
    binary_name = get_binary_name()
    src = project_dir / "target" / build_type / binary_name
    if not src.is_file():
        print(f"Binary not found: {src}", file=sys.stderr)
        return 1
    platform = get_platform()
    dst_dir = root / "bin" / platform
    dst_dir.mkdir(parents=True, exist_ok=True)
    dst = dst_dir / binary_name
    import shutil
    shutil.copy2(str(src), str(dst))
    if sys.platform != "win32":
        dst.chmod(dst.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    source_hash = sha256(src)
    bundled_hash = sha256(dst)
    if source_hash != bundled_hash:
        print(f"Artifact hash mismatch: {src} != {dst}", file=sys.stderr)
        return 1
    print(f"-> {dst}")
    print(f"sha256 {bundled_hash}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
