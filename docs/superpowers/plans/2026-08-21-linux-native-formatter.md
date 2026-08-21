# Linux Native Macro Formatter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the bundled native macro formatter executable and continuously verified on Linux x86_64 while preserving the Windows x86_64 path.

**Architecture:** Keep the existing bundled-binary model. Fix native platform detection and Unix permissions, make formatting converge before returning, extend the existing fixture runner into the acceptance oracle, and run the same checks on native Linux and Windows CI runners.

**Tech Stack:** Rust 2021, Python 3 standard library, TypeScript/VS Code extension, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-21-linux-native-formatter-design.md`

## Global Constraints

- Linux support in this plan is x86_64 only.
- Windows x86_64 path and `.exe` behavior must remain supported.
- Add no runtime downloads, install-time builds, or new package dependencies.
- Native failures must retain the existing fallback to ordinary `rustfmt`.
- Generated diagnostics belong under `rust-fmt-mf/target/macro-audit/`.

---

### Task 1: Correct native platform detection and Unix permissions

**Files:**
- Create: `scripts/test_build_current.py`
- Modify: `scripts/build_current.py`

**Interfaces:**
- Produces: `get_platform(sys_platform: str | None = None, machine: str | None = None) -> str`
- Produces: copied Unix binaries with user/group/other executable bits; Windows copies remain unchanged.

- [ ] **Step 1: Write the failing platform tests**

```python
import unittest

from scripts.build_current import get_platform


class PlatformTests(unittest.TestCase):
    def test_linux_x86_64(self):
        self.assertEqual(get_platform("linux", "x86_64"), "linux-x64")

    def test_windows_x86_64(self):
        self.assertEqual(get_platform("win32", "AMD64"), "win32-x64")

    def test_macos_arm64(self):
        self.assertEqual(get_platform("darwin", "arm64"), "darwin-arm64")
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `python3 -m unittest scripts/test_build_current.py -v`

Expected: ERROR because the current `get_platform` does not accept platform and machine arguments.

- [ ] **Step 3: Implement the minimal platform and mode fix**

Use `platform.machine()` and the existing standard library only:

```python
import platform
import stat


def get_platform(sys_platform: str | None = None, machine: str | None = None) -> str:
    sys_platform = sys_platform or sys.platform
    machine = machine or platform.machine()
    arch = {"AMD64": "x64", "x86_64": "x64", "arm64": "arm64", "aarch64": "arm64"}.get(machine)
    if arch is None:
        raise ValueError(f"Unsupported architecture: {machine}")
    os_name = "win32" if sys_platform == "win32" else "darwin" if sys_platform == "darwin" else "linux"
    return f"{os_name}-{arch}"
```

After `shutil.copy2`, add executable bits only when `sys.platform != "win32"`:

```python
if sys.platform != "win32":
    dst.chmod(dst.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
```

- [ ] **Step 4: Verify GREEN**

Run: `python3 -m unittest scripts/test_build_current.py -v`

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/build_current.py scripts/test_build_current.py
git commit -m "fix: detect native build platform correctly"
```

### Task 2: Make formatter output converge in one public call

**Files:**
- Modify: `rust-fmt-mf/tests/integration.rs`
- Modify: `rust-fmt-mf/src/lib.rs`
- Modify: `rust-fmt-mf/tests/fixtures/impl_for.expected`

**Interfaces:**
- Keeps: `format_source(source, rustfmt_path, edition, config_path) -> anyhow::Result<String>`
- Adds privately: `format_source_once(...) -> anyhow::Result<String>`

- [ ] **Step 1: Write the failing idempotence regression**

```rust
#[test]
fn test_impl_for_converges_in_one_call() {
    let source = include_str!("fixtures/impl_for.rs");
    let once = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    let twice = rust_fmt_mf::format_source(&once, "rustfmt", "2021", None).unwrap();
    assert_eq!(once, twice);
}
```

- [ ] **Step 2: Run the test and verify RED**

Run: `cargo test --test integration test_impl_for_converges_in_one_call -- --exact`

Expected: FAIL with indentation differences around `unimplemented!()`.

- [ ] **Step 3: Implement bounded fixed-point formatting**

Rename the current `format_source` body to private `format_source_once`. Make the public function run at most four passes and fail closed if output oscillates or does not converge:

```rust
pub fn format_source(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let mut current = source.to_string();
    for _ in 0..4 {
        let next = format_source_once(&current, rustfmt_path, edition, config_path)?;
        if next == current {
            return Ok(next);
        }
        current = next;
    }
    anyhow::bail!("macro formatting did not converge after 4 passes")
}
```

- [ ] **Step 4: Verify the regression and refresh the one changed golden file**

Run: `cargo test --test integration test_impl_for_converges_in_one_call -- --exact`

Expected: PASS.

Run `target/debug/rust-fmt-mf` on `tests/fixtures/impl_for.rs`, inspect that the second public call is identical, then update `impl_for.expected` to that converged output.

- [ ] **Step 5: Run the complete Rust suite and fixture corpus**

Run: `cargo test --all-targets`

Run: `python3 tests/run_fixtures.py`

Expected: 40 Rust tests plus the new regression pass; all 58 fixtures pass.

- [ ] **Step 6: Commit**

```bash
git add rust-fmt-mf/src/lib.rs rust-fmt-mf/tests/integration.rs rust-fmt-mf/tests/fixtures/impl_for.expected
git commit -m "fix: make macro formatting idempotent"
```

### Task 3: Turn the fixture runner into a binary acceptance oracle

**Files:**
- Create: `rust-fmt-mf/tests/test_run_fixtures.py`
- Modify: `rust-fmt-mf/tests/run_fixtures.py`

**Interfaces:**
- CLI: `python3 tests/run_fixtures.py [--binary PATH] [--rustfmt PATH]`
- Writes diagnostics to: `rust-fmt-mf/target/macro-audit/<fixture>/`

- [ ] **Step 1: Write failing CLI tests**

```python
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
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `python3 -m unittest discover -s tests -p 'test_*.py' -v`

Expected: ERROR because `parse_args` does not exist.

- [ ] **Step 3: Add argument parsing and exact binary selection**

Add `argparse` and expose:

```python
def parse_args(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--rustfmt", default="rustfmt")
    return parser.parse_args(argv)
```

Use the explicit path when supplied; otherwise retain `find_binary(root)`.

- [ ] **Step 4: Add the four acceptance checks**

For every fixture:

```python
first = subprocess.run([str(binary)], input=input_text, capture_output=True, text=True, encoding="utf-8")
syntax = subprocess.run([args.rustfmt, "--edition", "2021", "--emit", "stdout"], input=first.stdout, capture_output=True, text=True, encoding="utf-8")
second = subprocess.run([str(binary)], input=first.stdout, capture_output=True, text=True, encoding="utf-8")
```

Fail the case if the first process fails, normalized output differs from `.expected`, plain `rustfmt` fails, the second process fails, or `second.stdout != first.stdout`.

- [ ] **Step 5: Persist failure evidence**

At runner start, remove only `root / "target" / "macro-audit"`. For each failed case create its directory and write `input.rs`, `expected.rs`, `actual.rs`, `stderr.txt`, `rustfmt-stderr.txt`, `second.rs`, and `diff.patch` using `Path.write_text` and `difflib.unified_diff`.

- [ ] **Step 6: Verify the runner**

Run: `python3 -m unittest discover -s tests -p 'test_*.py' -v`

Run: `python3 tests/run_fixtures.py --binary target/debug/rust-fmt-mf`

Expected: Python tests pass, all 58 fixtures pass, and `target/macro-audit` is absent or empty.

- [ ] **Step 7: Commit**

```bash
git add rust-fmt-mf/tests/run_fixtures.py rust-fmt-mf/tests/test_run_fixtures.py
git commit -m "test: validate native macro formatter binaries"
```

### Task 4: Verify native Linux and Windows paths in CI

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `scripts/build_current.py`
- Consumes: `rust-fmt-mf/tests/run_fixtures.py --binary PATH`

- [ ] **Step 1: Add the native matrix job**

Add a job with `ubuntu-latest` and `windows-latest` entries. Its steps are checkout, stable Rust setup with `rustfmt`, `cargo test --all-targets`, `python scripts/build_current.py --release`, and the fixture runner against `${{ matrix.binary }}`.

Use matrix entries:

```yaml
include:
  - os: ubuntu-latest
    binary: bin/linux-x64/rust-fmt-mf
  - os: windows-latest
    binary: bin/win32-x64/rust-fmt-mf.exe
```

On Linux run `test -x bin/linux-x64/rust-fmt-mf`. On failure upload `rust-fmt-mf/target/macro-audit` with `actions/upload-artifact@v4` and `if-no-files-found: ignore`.

- [ ] **Step 2: Validate local commands represented by CI**

Run: `cargo test --all-targets` from `rust-fmt-mf/`.

Run: `python3 scripts/build_current.py --release` from the repository root.

Run: `python3 rust-fmt-mf/tests/run_fixtures.py --binary bin/linux-x64/rust-fmt-mf`.

Expected: all commands pass on Linux; Windows commands are the same paths with `.exe` and run on `windows-latest`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: test native formatter on Linux and Windows"
```

### Task 5: Refresh and verify the bundled Linux artifact

**Files:**
- Modify: `bin/linux-x64/rust-fmt-mf` including Git executable mode.
- Preserve unchanged: `bin/win32-x64/rust-fmt-mf.exe`

**Interfaces:**
- Produces the exact Linux binary selected by `getNativeMacroFormatterPath`.

- [ ] **Step 1: Record the Windows artifact hash**

Run: `sha256sum bin/win32-x64/rust-fmt-mf.exe`

Keep the value for the final unchanged check.

- [ ] **Step 2: Build the Linux release artifact**

Run: `python3 scripts/build_current.py --release`

Expected: `bin/linux-x64/rust-fmt-mf` is replaced from `rust-fmt-mf/target/release/rust-fmt-mf`.

- [ ] **Step 3: Verify mode and behavior**

Run: `test -x bin/linux-x64/rust-fmt-mf`

Run: `git ls-files -s bin/linux-x64/rust-fmt-mf`

Expected Git mode after staging: `100755`.

Run: `python3 rust-fmt-mf/tests/run_fixtures.py --binary bin/linux-x64/rust-fmt-mf`

Expected: all 58 fixtures pass.

- [ ] **Step 4: Verify Windows remains unchanged**

Run: `sha256sum bin/win32-x64/rust-fmt-mf.exe`

Expected: identical to Step 1.

- [ ] **Step 5: Commit**

```bash
git add bin/linux-x64/rust-fmt-mf
git commit -m "build: refresh Linux macro formatter binary"
```

### Task 6: Final verification

**Files:**
- Verify only.

- [ ] **Step 1: Run all local checks**

```bash
python3 -m unittest scripts/test_build_current.py -v
cd rust-fmt-mf && cargo fmt --all -- --check
cd rust-fmt-mf && cargo test --all-targets
cd rust-fmt-mf && python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 rust-fmt-mf/tests/run_fixtures.py --binary bin/linux-x64/rust-fmt-mf
test -x bin/linux-x64/rust-fmt-mf
git diff --check
git status --short
```

Expected: every command exits 0; only intended commits differ from the starting revision; Windows binary hash remains unchanged.
