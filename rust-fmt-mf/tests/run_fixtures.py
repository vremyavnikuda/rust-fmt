#!/usr/bin/env python3
"""Audit every golden fixture and every Rust source in test-rs."""

import argparse
import difflib
import shutil
import subprocess
import sys
from pathlib import Path


CORPUS_GOLDENS = {
    Path("src/examples/macro_edge_cases.rs"): "real_macro_edge_cases.expected",
    Path("src/examples/macro_heavy.rs"): "real_macro_heavy.expected",
    Path("src/examples/macro_missing_cases.rs"): "real_macro_missing_cases.expected",
    Path("src/main_fmt.rs"): "real_main_fmt.expected",
}


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def find_binary(root: Path) -> Path:
    for profile in ("release", "debug"):
        for name in ("rust-fmt-mf.exe", "rust-fmt-mf"):
            candidate = root / "target" / profile / name
            if candidate.is_file():
                return candidate
    raise FileNotFoundError("build rust-fmt-mf before running the audit")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--rustfmt", default="rustfmt")
    parser.add_argument("--skip-corpus", action="store_true")
    return parser.parse_args(argv)


def corpus_expected_path(relative: Path, fixture_dir: Path) -> Path | None:
    filename = CORPUS_GOLDENS.get(relative)
    return fixture_dir / filename if filename is not None else None


def run(command: list[str], source: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        input=source,
        capture_output=True,
        text=True,
        encoding="utf-8",
        cwd=cwd,
    )


def parse_outcomes(stderr: str) -> list[tuple[str, str, str]]:
    outcomes = []
    for line in stderr.splitlines():
        fields = line.split("\t", 4)
        if len(fields) >= 4 and fields[0] == "rust-fmt-mf":
            outcomes.append((fields[1], fields[2], fields[4] if len(fields) == 5 else ""))
    return outcomes


def count_macro_outcomes(outcomes: list[tuple[str, str, str]]) -> tuple[int, int, int]:
    total = len(outcomes)
    handled = sum(status != "SKIPPED" for status, _, _ in outcomes)
    formatted = sum(status == "FORMATTED" for status, _, _ in outcomes)
    return total, handled, formatted


def golden_failure(expected: str, actual: str) -> str | None:
    if actual == expected:
        return None

    def without_blank_lines(source: str) -> str:
        return "".join(
            line for line in source.splitlines(keepends=True) if line.strip()
        )

    if without_blank_lines(actual) == without_blank_lines(expected):
        return "GOLDEN_BLANK_LINES"
    return "GOLDEN_DIFF"


def expected_skips(input_path: Path) -> set[str]:
    sidecar = input_path.with_suffix(".skipped")
    if not sidecar.is_file():
        return set()
    return {
        line.strip()
        for line in sidecar.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    }


def format_flags(input_path: Path) -> list[str]:
    """Extra rust-fmt-mf flags for this fixture.

    A `.compact` sidecar marks a fixture whose golden was produced with
    blank-line compaction on. Compaction is off by default, so without the
    marker these fixtures would only differ from their golden by the blank
    lines the rule removes -- and their goldens cannot simply be regenerated,
    because CORPUS_GOLDENS shares them with the already-compacted test-rs
    sources.
    """
    return ["--compact-blank-lines"] if input_path.with_suffix(".compact").is_file() else []


def safe_name(prefix: str, path: Path) -> str:
    return prefix + "__" + "__".join(path.parts).replace(".rs", "")


def write_failure(
    audit_root: Path,
    name: str,
    input_text: str,
    actual: str,
    stderr: str,
    second: str,
    expected: str | None,
    rustfmt_stderr: str,
) -> None:
    case_dir = audit_root / name
    case_dir.mkdir(parents=True, exist_ok=True)
    files = {
        "input.rs": input_text,
        "actual.rs": actual,
        "stderr.txt": stderr,
        "second.rs": second,
        "rustfmt-stderr.txt": rustfmt_stderr,
    }
    if expected is not None:
        files["expected.rs"] = expected
        files["diff.patch"] = "".join(
            difflib.unified_diff(
                expected.splitlines(keepends=True),
                actual.splitlines(keepends=True),
                fromfile="expected",
                tofile="actual",
            )
        )
    for filename, content in files.items():
        (case_dir / filename).write_text(content, encoding="utf-8")


def audit_case(
    binary: Path,
    rustfmt: str,
    input_path: Path,
    expected: str | None,
    audit_root: Path,
    audit_name: str,
    enforce_expected_skips: bool,
) -> dict[str, object]:
    source = input_path.read_text(encoding="utf-8")
    command = [str(binary), *format_flags(input_path)]
    failures: list[str] = []
    try:
        first = run(command, source)
    except OSError as error:
        first = subprocess.CompletedProcess(command, 127, "", str(error))

    if first.returncode != 0:
        failures.append("FORMAT_ERROR")
        syntax = subprocess.CompletedProcess([], 1, "", "not run")
        second = subprocess.CompletedProcess([], 1, "", "not run")
        outcomes: list[tuple[str, str, str]] = []
    else:
        syntax = run(
            [
                rustfmt,
                "--edition",
                "2021",
                "--emit",
                "stdout",
                "--config",
                "format_macro_bodies=false",
                "--config",
                "format_macro_matchers=false",
            ],
            first.stdout,
        )
        second = run(command, first.stdout)
        outcomes = parse_outcomes(first.stderr)
        if syntax.returncode != 0:
            failures.append("SYNTAX_ERROR")
        elif expected is None and syntax.stdout != first.stdout:
            failures.append("RUSTFMT_DIFF")
        if second.returncode != 0 or second.stdout != first.stdout:
            failures.append("NON_IDEMPOTENT")

    if expected is not None:
        mismatch = golden_failure(expected, first.stdout)
        if mismatch is not None:
            failures.append(mismatch)

    skipped = {name for status, name, _ in outcomes if status == "SKIPPED"}
    expected_skip_names = expected_skips(input_path) if enforce_expected_skips else skipped
    if enforce_expected_skips:
        if skipped - expected_skip_names:
            failures.append("UNEXPECTED_SKIPPED")
        if expected_skip_names - skipped:
            failures.append("MISSING_SKIPPED")

    if failures:
        write_failure(
            audit_root,
            audit_name,
            source,
            first.stdout,
            first.stderr + second.stderr,
            second.stdout,
            expected,
            syntax.stderr,
        )

    return {
        "failures": failures,
        "output": first.stdout,
        "outcomes": outcomes,
        "safe": not any(item in failures for item in ("FORMAT_ERROR", "SYNTAX_ERROR", "NON_IDEMPOTENT")),
        "exact": None if expected is None else first.stdout == expected,
    }


def percent(passed: int, total: int) -> str:
    return f"{(100.0 * passed / total) if total else 100.0:.1f}%"


def main() -> int:
    args = parse_args()
    root = project_root()
    repo = root.parent
    binary = (args.binary or find_binary(root)).resolve()
    if not binary.is_file():
        print(f"Binary not found: {binary}", file=sys.stderr)
        return 1

    fixture_dir = root / "tests" / "fixtures"
    inputs = sorted(fixture_dir.glob("*.rs"))
    expecteds = {path.stem: path for path in fixture_dir.glob("*.expected")}
    missing_expected = [path.name for path in inputs if path.stem not in expecteds]
    orphan_expected = [path.name for name, path in expecteds.items() if not (fixture_dir / f"{name}.rs").is_file()]
    if missing_expected or orphan_expected:
        for name in missing_expected:
            print(f"MISSING_EXPECTED {name}")
        for name in orphan_expected:
            print(f"ORPHAN_EXPECTED {name}")
        return 1

    audit_root = root / "target" / "macro-audit"
    if audit_root.exists():
        shutil.rmtree(audit_root)
    audit_root.mkdir(parents=True)

    total_cases = safe_cases = 0
    exact_total = exact_passed = 0
    macro_total = macro_handled = macro_formatted = 0
    failed = False

    print(f"Discovered {len(inputs)} golden fixtures")
    for input_path in inputs:
        expected = expecteds[input_path.stem].read_text(encoding="utf-8")
        result = audit_case(binary, args.rustfmt, input_path, expected, audit_root, input_path.stem, True)
        total_cases += 1
        safe_cases += int(result["safe"])
        exact_total += 1
        exact_passed += int(result["exact"])
        outcomes = result["outcomes"]
        case_total, case_handled, case_formatted = count_macro_outcomes(outcomes)
        macro_total += case_total
        macro_handled += case_handled
        macro_formatted += case_formatted
        failures = result["failures"]
        print(f"{input_path.stem}: {'PASS' if not failures else ', '.join(failures)}")
        failed |= bool(failures)

    if not args.skip_corpus:
        corpus_root = repo / "test-rs"
        formatted_corpus = audit_root / "formatted-test-rs"
        shutil.copytree(corpus_root, formatted_corpus)
        corpus_files = sorted((corpus_root / "src").rglob("*.rs"))
        print(f"Discovered {len(corpus_files)} test-rs source files")
        for input_path in corpus_files:
            relative = input_path.relative_to(corpus_root)
            expected_path = corpus_expected_path(relative, fixture_dir)
            expected = (
                expected_path.read_text(encoding="utf-8")
                if expected_path is not None
                else None
            )
            result = audit_case(
                binary,
                args.rustfmt,
                input_path,
                expected,
                audit_root,
                safe_name("corpus", relative),
                False,
            )
            total_cases += 1
            safe_cases += int(result["safe"])
            if expected is not None:
                exact_total += 1
                exact_passed += int(result["exact"])
            outcomes = result["outcomes"]
            case_total, case_handled, case_formatted = count_macro_outcomes(outcomes)
            macro_total += case_total
            macro_handled += case_handled
            macro_formatted += case_formatted
            failures = result["failures"]
            print(f"test-rs/{relative}: {'PASS' if not failures else ', '.join(failures)}")
            failed |= bool(failures)
            if result["safe"]:
                (formatted_corpus / relative).write_text(str(result["output"]), encoding="utf-8")

        check = subprocess.run(
            ["cargo", "check", "--all-targets"],
            cwd=formatted_corpus,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        (audit_root / "cargo-check.txt").write_text(check.stdout + check.stderr, encoding="utf-8")
        print(f"formatted test-rs cargo check: {'PASS' if check.returncode == 0 else 'SYNTAX_ERROR'}")
        failed |= check.returncode != 0

    print()
    print(f"Execution safety: {safe_cases}/{total_cases} ({percent(safe_cases, total_cases)})")
    print(
        f"Exact-output conformance: {exact_passed}/{exact_total} "
        f"({percent(exact_passed, exact_total)})"
    )
    print(
        f"Macros handled without skip: {macro_handled}/{macro_total} "
        f"({percent(macro_handled, macro_total)})"
    )
    print(f"Macros changed on this input: {macro_formatted}/{macro_total}")
    print(f"Diagnostics: {audit_root}")
    return int(failed or safe_cases != total_cases or exact_passed != exact_total)


if __name__ == "__main__":
    sys.exit(main())
