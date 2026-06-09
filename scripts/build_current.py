#!/usr/bin/env python3
"""Build rust-fmt-mf for the current platform and copy to bin/.

Usage:
    python scripts/build_current.py               # release build
    python scripts/build_current.py --debug        # debug build
    python scripts/build_current.py --release      # explicit release build
"""

import argparse
import subprocess
import sys
from pathlib import Path


def get_project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def get_binary_name() -> str:
    return "rust-fmt-mf.exe" if sys.platform == "win32" else "rust-fmt-mf"


def get_platform() -> str:
    arch_map = {"AMD64": "x64", "x86_64": "x64", "arm64": "arm64", "aarch64": "arm64"}
    arch = arch_map.get(sys.platform == "win32" and "AMD64" or "x86_64", "x64")
    if sys.platform == "win32":
        return f"win32-{arch}"
    elif sys.platform == "darwin":
        return f"darwin-{arch}"
    else:
        return f"linux-{arch}"


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
    print(f"-> {dst}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
