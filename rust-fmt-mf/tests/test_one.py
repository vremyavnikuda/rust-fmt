#!/usr/bin/env python3
"""Quick test: format a single fixture and show input/output/expected.

Usage:
    python tests/test_one.py [fixture_name]

Default fixture: "simple"
"""

import subprocess
import sys
from pathlib import Path


def get_project_root() -> Path:
    script = Path(__file__).resolve()
    if script.parent.name == "tests":
        return script.parent.parent
    return script.parent


def find_binary(project_root: Path) -> Path:
    candidates = [
        project_root / "target" / "release" / "rust-fmt-mf.exe",
        project_root / "target" / "release" / "rust-fmt-mf",
        project_root / "target" / "debug" / "rust-fmt-mf.exe",
        project_root / "target" / "debug" / "rust-fmt-mf",
    ]
    for c in candidates:
        if c.is_file():
            return c
    sys.exit(f"Binary not found. Build first with: cargo build -p rust-fmt-mf")


def cyan(s: str) -> str:
    return f"\033[96m{s}\033[0m"


def yellow(s: str) -> str:
    return f"\033[93m{s}\033[0m"


def green(s: str) -> str:
    return f"\033[92m{s}\033[0m"


def main() -> int:
    name = sys.argv[1] if len(sys.argv) > 1 else "simple"
    root = get_project_root()
    binary = find_binary(root)
    fixture_dir = root / "tests" / "fixtures"
    input_path = fixture_dir / f"{name}.rs"
    expected_path = fixture_dir / f"{name}.expected"
    if not input_path.is_file():
        print(f"Fixture not found: {input_path}")
        return 1
    input_text = input_path.read_text(encoding="utf-8")
    expected = expected_path.read_text(encoding="utf-8") if expected_path.is_file() else "(no expected file)"
    print(f"{cyan('Input:')}")
    print(input_text)
    print()
    proc = subprocess.run(
        [str(binary)],
        input=input_text,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if proc.returncode != 0:
        print(f"ERROR (exit code {proc.returncode}):")
        print(proc.stderr)
        return 1
    print(f"{yellow('Output:')}")
    print(proc.stdout)
    print()
    print(f"{green('Expected:')}")
    print(expected)
    return 0


if __name__ == "__main__":
    sys.exit(main())
