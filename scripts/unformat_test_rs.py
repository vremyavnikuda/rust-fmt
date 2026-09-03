#!/usr/bin/env python3
"""Mangle whitespace in test-rs/src/**/*.rs so the formatter has something to fix.

Only whitespace is touched - indentation, spaces around punctuation, and where
lines break - so the code still compiles and a correct formatter must restore
the committed version exactly:

    python scripts/unformat_test_rs.py       # default seed
    python scripts/unformat_test_rs.py 42    # a different mangling
    # format in VS Code / with rust-fmt-mf, then:
    git diff --stat test-rs        # should be empty

Restore without formatting: git checkout -- test-rs
"""

import random
import re
import sys
from pathlib import Path

INDENTS = ["", " ", "  ", "    ", "\t", "\t\t", "\t    ", "        ", "           "]
# ponytail: line-level heuristics instead of a lexer; blank lines are never
# touched because rustfmt preserves them, so they keep the diff an exact oracle.
UNSAFE = re.compile(r"""["']|//|/\*|\*/""")
SQUEEZE = re.compile(r"\s*([=<>,;:+&|(){}\[\]-])\s*")
SPLIT_AT = re.compile(r"(?<=,) ")


def safe(line: str) -> bool:
    return UNSAFE.search(line) is None


def squeeze(lines: list[str], rng: random.Random) -> list[str]:
    return [
        SQUEEZE.sub(r"\1", l) if safe(l) and rng.random() < 0.85 else l
        for l in lines
    ]


def split(lines: list[str], rng: random.Random) -> list[str]:
    out = []
    for line in lines:
        cuts = [m.start() for m in SPLIT_AT.finditer(line)]
        if cuts and safe(line) and rng.random() < 0.3:
            cut = rng.choice(cuts)
            out.extend([line[:cut], line[cut + 1:]])
        else:
            out.append(line)
    return out


def join(lines: list[str], rng: random.Random) -> list[str]:
    out = []
    for line in lines:
        prev = out[-1] if out else ""
        if prev.strip() and line.strip() and safe(prev) and safe(line) and rng.random() < 0.35:
            out[-1] = prev + " " + line.strip()
        else:
            out.append(line)
    return out


def indent(lines: list[str], rng: random.Random) -> list[str]:
    return [
        rng.choice(INDENTS) + l.strip() if l.strip() else ""
        for l in lines
    ]


def main() -> int:
    seed = int(sys.argv[1], 0) if len(sys.argv) > 1 else 0xC0FFEE
    root = Path(__file__).resolve().parent.parent / "test-rs" / "src"
    rng = random.Random(seed)
    files = sorted(root.rglob("*.rs"))
    if not files:
        print(f"No .rs files under {root}", file=sys.stderr)
        return 1
    for path in files:
        lines = path.read_text(encoding="utf-8").splitlines()
        for step in (squeeze, split, join, indent):
            lines = step(lines, rng)
        path.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
        print(f"mangled {path.relative_to(root.parent)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
