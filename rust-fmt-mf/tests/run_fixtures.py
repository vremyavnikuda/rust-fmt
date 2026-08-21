#!/usr/bin/env python3
"""Test runner for rust-fmt-mf fixtures.
Runs each .rs fixture through the binary and compares output to .expected.

Usage:
    python tests/run_fixtures.py                  # run from project root
    python run_fixtures.py                        # run from tests/ dir
"""

import argparse
import difflib
import shutil
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


def parse_args(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--rustfmt", default="rustfmt")
    return parser.parse_args(argv)


FIXTURES = [
    "async_body",
    "attr_pat",
    "bracket_arm_pattern",
    "bracket_pattern",
    "comments",
    "complex_pattern",
    "define_enum_invocation",
    "derive_builder_with_state",
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


def write_failure(
    audit_root: Path,
    name: str,
    input_text: str,
    expected: str,
    actual: str,
    stderr: str,
    rustfmt_stderr: str,
    second: str,
) -> None:
    case_dir = audit_root / name
    case_dir.mkdir(parents=True, exist_ok=True)
    diff = "".join(
        difflib.unified_diff(
            normalize(expected).splitlines(keepends=True),
            normalize(actual).splitlines(keepends=True),
            fromfile="expected",
            tofile="actual",
        )
    )
    for filename, content in {
        "input.rs": input_text,
        "expected.rs": expected,
        "actual.rs": actual,
        "stderr.txt": stderr,
        "rustfmt-stderr.txt": rustfmt_stderr,
        "second.rs": second,
        "diff.patch": diff,
    }.items():
        (case_dir / filename).write_text(content, encoding="utf-8")


def main() -> int:
    args = parse_args()
    root = get_project_root()
    binary = args.binary.resolve() if args.binary else find_binary(root)
    if not binary.is_file():
        print(red(f"Binary not found: {binary}"), file=sys.stderr)
        return 1
    fixture_dir = root / "tests" / "fixtures"
    audit_root = root / "target" / "macro-audit"
    if audit_root.exists():
        shutil.rmtree(audit_root)
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
        try:
            proc = subprocess.run(
                [str(binary)],
                input=input_text,
                capture_output=True,
                text=True,
                encoding="utf-8",
            )
        except OSError as exc:
            print(red(f"ERROR: {exc}"))
            write_failure(audit_root, name, input_text, expected, "", str(exc), "", "")
            all_passed = False
            continue
        syntax = subprocess.run(
            [args.rustfmt, "--edition", "2021", "--emit", "stdout"],
            input=proc.stdout,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        second = subprocess.run(
            [str(binary)],
            input=proc.stdout,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        result = normalize(proc.stdout)
        expected_norm = normalize(expected)
        failures = []
        if proc.returncode != 0:
            failures.append(f"formatter exit {proc.returncode}")
        if result != expected_norm:
            failures.append("golden output differs")
        if syntax.returncode != 0:
            failures.append(f"rustfmt validation exit {syntax.returncode}")
        if second.returncode != 0:
            failures.append(f"second formatter exit {second.returncode}")
        elif second.stdout != proc.stdout:
            failures.append("output is not idempotent")
        if not failures:
            print(green("PASS"))
        else:
            print(red("FAIL: " + ", ".join(failures)))
            diff = difflib.unified_diff(
                expected_norm.splitlines(),
                result.splitlines(),
                fromfile="expected",
                tofile="actual",
                lineterm="",
            )
            for line in diff:
                print(gray(line))
            write_failure(
                audit_root,
                name,
                input_text,
                expected,
                proc.stdout,
                proc.stderr + second.stderr,
                syntax.stderr,
                second.stdout,
            )
            all_passed = False
    if all_passed:
        print(f"\n{green('All fixtures passed!')}")
        return 0
    else:
        print(f"\n{red('Some fixtures failed.')}")
        return 1

if __name__ == "__main__":
    sys.exit(main())
