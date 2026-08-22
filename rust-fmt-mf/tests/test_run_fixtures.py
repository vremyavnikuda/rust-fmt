import unittest
from subprocess import CompletedProcess
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

import run_fixtures


class ArgumentTests(unittest.TestCase):
    def test_explicit_binary(self):
        args = run_fixtures.parse_args(["--binary", "/tmp/rust-fmt-mf"])
        self.assertEqual(args.binary, Path("/tmp/rust-fmt-mf"))

    def test_default_rustfmt(self):
        args = run_fixtures.parse_args([])
        self.assertEqual(args.rustfmt, "rustfmt")


class CorpusGoldenTests(unittest.TestCase):
    def test_real_macro_files_use_user_approved_goldens(self):
        fixture_dir = Path("/fixtures")

        expected = {
            Path("src/examples/macro_edge_cases.rs"): "real_macro_edge_cases.expected",
            Path("src/examples/macro_heavy.rs"): "real_macro_heavy.expected",
            Path("src/examples/macro_missing_cases.rs"): "real_macro_missing_cases.expected",
            Path("src/main_fmt.rs"): "real_main_fmt.expected",
        }
        self.assertEqual(run_fixtures.CORPUS_GOLDENS, expected)
        for relative, filename in expected.items():
            self.assertEqual(
                run_fixtures.corpus_expected_path(relative, fixture_dir),
                fixture_dir / filename,
            )
        self.assertIsNone(
            run_fixtures.corpus_expected_path(Path("src/lib.rs"), fixture_dir)
        )

    def test_golden_mismatch_is_a_failure(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "input.rs"
            input_path.write_text("fn main() {}\n", encoding="utf-8")
            formatted = CompletedProcess([], 0, "fn main() {}\n", "")
            syntax = CompletedProcess([], 0, "fn main() {}\n", "")
            idempotent = CompletedProcess([], 0, "fn main() {}\n", "")

            with patch.object(
                run_fixtures, "run", side_effect=[formatted, syntax, idempotent]
            ):
                result = run_fixtures.audit_case(
                    Path("formatter"),
                    "rustfmt",
                    input_path,
                    "fn main() { println!(\"expected\"); }\n",
                    root / "audit",
                    "case",
                    False,
                )

        self.assertIn("GOLDEN_DIFF", result["failures"])
        self.assertFalse(result["exact"])

    def test_blank_line_only_mismatch_has_a_specific_failure(self):
        expected = "fn main() {\n    first();\n\n    second();\n}\n"
        actual = "fn main() {\n    first();\n    second();\n}\n"

        self.assertEqual(
            run_fixtures.golden_failure(expected, actual), "GOLDEN_BLANK_LINES"
        )

    def test_corpus_without_golden_must_match_rustfmt(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "input.rs"
            input_path.write_text("fn main(){work();}\n", encoding="utf-8")
            formatted = CompletedProcess([], 0, "fn main() {\n    work();\n}\n", "")
            rustfmt = CompletedProcess([], 0, "fn main() { work(); }\n", "")
            idempotent = CompletedProcess([], 0, formatted.stdout, "")

            with patch.object(
                run_fixtures, "run", side_effect=[formatted, rustfmt, idempotent]
            ):
                result = run_fixtures.audit_case(
                    Path("formatter"),
                    "rustfmt",
                    input_path,
                    None,
                    root / "audit",
                    "case",
                    False,
                )

        self.assertIn("RUSTFMT_DIFF", result["failures"])


class MacroMetricTests(unittest.TestCase):
    def test_unchanged_is_handled_but_not_formatted(self):
        outcomes = [
            ("FORMATTED", "changed", ""),
            ("UNCHANGED", "stable", ""),
            ("SKIPPED", "unsupported", "reason"),
        ]

        self.assertEqual(run_fixtures.count_macro_outcomes(outcomes), (3, 2, 1))


if __name__ == "__main__":
    unittest.main()
