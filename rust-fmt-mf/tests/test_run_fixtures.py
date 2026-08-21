import unittest
from pathlib import Path

import run_fixtures


class ArgumentTests(unittest.TestCase):
    def test_explicit_binary(self):
        args = run_fixtures.parse_args(["--binary", "/tmp/rust-fmt-mf"])
        self.assertEqual(args.binary, Path("/tmp/rust-fmt-mf"))

    def test_default_rustfmt(self):
        args = run_fixtures.parse_args([])
        self.assertEqual(args.rustfmt, "rustfmt")


if __name__ == "__main__":
    unittest.main()
