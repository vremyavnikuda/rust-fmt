#!/usr/bin/env python3
"""Test runner for rust-fmt-mf fixtures.
Runs each .rs fixture through the binary and compares output to .expected.

Usage:
    python tests/run_fixtures.py                  # run from project root
    python run_fixtures.py                        # run from tests/ dir
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


FIXTURES = [
    "async_body",
    "attr_pat",
    "bracket_arm_pattern",
    "bracket_pattern",
    "comments",
    "complex_pattern",
    "define_enum_invocation",
    "dispatch",
    "dollar_crate",
    "dollar_dollar",
    "double_brace",
    "empty_body",
    "field_accessor",
    "field_accessor_invocation",
    "long_expr",
    "macro_export_doc",
    "match_closure",
    "mixed_delims",
    "mixed_macros",
    "multi_arm",
    "multi_crate",
    "multi_line_pat",
    "nested",
    "nested_diff_sep",
    "no_rep",
    "optional_question",
    "pat_literal_string",
    "recursive_invocation",
    "rle_invocation",
    "semi_sep",
    "simple",
    "single_token",
    "star_plus_mix",
    "string_body",
    "triple_nested",
    "tt_dispatch_invocation",
    "tt_munching",
    "two_reps",
    "unsafe_block",
    "var_in_rep",
    "vec_of_strings",
    "vis_fn",
    "closure_move",
    "const_static",
    "extern_c",
    "for_loop_body",
    "impl_for",
    "match_gen_arm",
    "stringify_concat",
    "struct_with_bounds",
    "try_op",
]


def green(s: str) -> str:
    return f"\033[92m{s}\033[0m"


def red(s: str) -> str:
    return f"\033[91m{s}\033[0m"


def cyan(s: str) -> str:
    return f"\033[96m{s}\033[0m"


def yellow(s: str) -> str:
    return f"\033[93m{s}\033[0m"


def gray(s: str) -> str:
    return f"\033[90m{s}\033[0m"


def normalize(s: str) -> str:
    return s.replace("\r\n", "\n").strip() + "\n"


def main() -> int:
    root = get_project_root()
    binary = find_binary(root)
    fixture_dir = root / "tests" / "fixtures"
    all_passed = True
    for name in FIXTURES:
        input_path = fixture_dir / f"{name}.rs"
        expected_path = fixture_dir / f"{name}.expected"
        if not input_path.is_file() or not expected_path.is_file():
            print(f"{name}  {red('MISSING')}")
            all_passed = False
            continue
        input_text = input_path.read_text(encoding="utf-8")
        expected = expected_path.read_text(encoding="utf-8")
        print(f"{name}", end="  ")
        proc = subprocess.run(
            [str(binary)],
            input=input_text,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if proc.returncode != 0:
            print(red("ERROR"))
            print(proc.stderr)
            all_passed = False
            continue
        result = normalize(proc.stdout)
        expected_norm = normalize(expected)
        if result == expected_norm:
            print(green("PASS"))
        else:
            print(red("DIFF:"))
            print(gray("  Expected:"))
            for line in expected.splitlines():
                print(f"    {line}")
            print(gray("  Got:"))
            for line in proc.stdout.splitlines():
                print(f"    {line}")
            all_passed = False
    if all_passed:
        print(f"\n{green('All fixtures passed!')}")
        return 0
    else:
        print(f"\n{red('Some fixtures failed.')}")
        return 1

if __name__ == "__main__":
    sys.exit(main())
