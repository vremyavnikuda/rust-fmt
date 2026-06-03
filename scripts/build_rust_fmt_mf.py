#!/usr/bin/env python3
"""Build rust-fmt-mf for one or more targets and copy binaries to bin/.

Usage:
    python scripts/build_rust_fmt_mf.py                    # build current platform only (release)
    python scripts/build_rust_fmt_mf.py --debug             # build current platform (debug)
    python scripts/build_rust_fmt_mf.py --all               # try all known targets
    python scripts/build_rust_fmt_mf.py --target x86_64-unknown-linux-gnu --platform linux-x64
"""

import argparse
import subprocess
import sys
from pathlib import Path


def get_project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def get_binary_name(platform: str) -> str:
    return "rust-fmt-mf.exe" if platform.startswith("win32") else "rust-fmt-mf"


def get_current_platform() -> str:
    arch = "x64"
    if sys.platform == "win32":
        return "win32-x64"
    elif sys.platform == "darwin":
        return "darwin-x64"
    else:
        return "linux-x64"


def get_current_target() -> str:
    if sys.platform == "win32":
        return "x86_64-pc-windows-msvc"
    elif sys.platform == "darwin":
        return "x86_64-apple-darwin"
    else:
        return "x86_64-unknown-linux-gnu"


ALL_TARGETS: list[dict[str, str]] = [
    {"target": "x86_64-pc-windows-msvc", "platform": "win32-x64"},
    {"target": "x86_64-unknown-linux-gnu", "platform": "linux-x64"},
    {"target": "x86_64-apple-darwin", "platform": "darwin-x64"},
    {"target": "aarch64-apple-darwin", "platform": "darwin-arm64"},
]


def build_target(
    target: str,
    platform: str,
    project_dir: Path,
    bin_dir: Path,
    is_release: bool,
) -> bool:
    build_type = "release" if is_release else "debug"
    profile_flag = "--release" if is_release else ""
    print(f"Building for {target} ({platform})...")
    cmd = ["cargo", "build", "-p", "rust-fmt-mf", "--manifest-path", str(project_dir / "Cargo.toml")]
    cmd += ["--target", target]
    if profile_flag:
        cmd.append(profile_flag)
    result = subprocess.run(cmd)
    if result.returncode != 0:
        print(f"  Build failed for {target}", file=sys.stderr)
        return False
    binary_name = get_binary_name(platform)
    src = project_dir / "target" / target / build_type / binary_name
    if not src.is_file():
        print(f"  Binary not found: {src}", file=sys.stderr)
        return False
    dst_dir = bin_dir / platform
    dst_dir.mkdir(parents=True, exist_ok=True)
    dst = dst_dir / binary_name
    import shutil
    shutil.copy2(str(src), str(dst))
    print(f"  -> {dst}")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Build rust-fmt-mf for one or more targets")
    parser.add_argument("--release", action="store_true", help="Release build (default)")
    parser.add_argument("--debug", action="store_true", help="Debug build")
    parser.add_argument("--all", action="store_true", help="Build for all known targets")
    parser.add_argument("--target", type=str, help="Rust target triple (e.g. x86_64-unknown-linux-gnu)")
    parser.add_argument("--platform", type=str, help="Platform directory name (e.g. linux-x64)")
    parser.add_argument("--skip-current", action="store_true", help="Skip current platform when using --all")
    args = parser.parse_args()
    is_release = not args.debug
    build_type = "release" if is_release else "debug"
    root = get_project_root()
    project_dir = root / "rust-fmt-mf"
    bin_dir = root / "bin"
    targets_to_build: list[dict[str, str]] = []
    if args.target:
        if not args.platform:
            print("--platform is required when using --target", file=sys.stderr)
            return 1
        targets_to_build.append({"target": args.target, "platform": args.platform})
    elif args.all:
        current_platform = get_current_platform()
        for t in ALL_TARGETS:
            if args.skip_current and t["platform"] == current_platform:
                continue
            targets_to_build.append(t)
    else:
        targets_to_build.append({"target": get_current_target(), "platform": get_current_platform()})
    print(f"Building rust-fmt-mf ({build_type})")
    print(f"Current platform: {get_current_platform()}")
    success = True
    for t in targets_to_build:
        if not build_target(t["target"], t["platform"], project_dir, bin_dir, is_release):
            success = False
    if not args.all and not args.target:
        print()
        print("Cross-compilation targets (use --all or specify --target/--platform):")
        print("  Install with: rustup target add x86_64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin")
    print(f"\nDone! Binaries placed in {bin_dir}")
    return 0 if success else 1

if __name__ == "__main__":
    sys.exit(main())
