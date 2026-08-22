# Safe Macro Formatting and Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the native macro formatter lossless, idempotent, fully audited, and fail-safe while preserving Linux and Windows extension behavior.

**Architecture:** Replace character heuristics with a Rust-aware lexical model, keep all rewrites range-based, and validate every result with a token-preservation oracle. The extension sends original text to the native engine and safely falls back to ordinary `rustfmt`; the acceptance runner discovers the complete corpus and reports safety, golden, and deep-format coverage separately.

**Tech Stack:** Rust 2021, `ra-ap-rustc_lexer` 0.174.0, stable `rustfmt`, TypeScript, Python 3 standard library, VS Code extension host, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-21-macro-formatting-safety-design.md`

## Global Constraints

- Safety coverage must be 100%: no changed significant tokens, invalid Rust, crashes, or non-idempotent output.
- Unsupported opaque DSL is preserved byte-for-byte and reported as `SKIPPED`.
- TypeScript must never rewrite Rust text heuristically before native formatting or fallback.
- Linux x86_64 and Windows x86_64 bundled paths remain supported.
- Do not modify or reset the user's existing `test-rs` edits.
- Do not create Git commits; commit steps are intentionally omitted by user request.
- Runtime downloads and install-time Rust compilation remain out of scope.

---

### Task 1: Rust-aware macro and arm spans

**Files:**
- Modify: `rust-fmt-mf/Cargo.toml`
- Modify: `rust-fmt-mf/Cargo.lock`
- Replace: `rust-fmt-mf/src/parser.rs`
- Modify: `rust-fmt-mf/src/tests.rs`

**Interfaces:**
- Produces: `parse_macro_defs(source: &str) -> anyhow::Result<Vec<MacroDef>>`
- Produces: `significant_tokens(source: &str) -> anyhow::Result<Vec<SignificantToken>>`
- Produces: `SignificantToken { kind: String, text: String, span: Range<usize> }`
- Preserves: existing `MacroDef` and `MacroArm` byte-range consumers.

- [ ] **Step 1: Write failing parser regressions**

Add tests proving that string/comment lookalikes are ignored, Unicode ranges remain valid, delimiters inside literals/comments do not close a macro, and all legal outer/transcriber delimiters are found:

~~~rust
#[test]
fn parser_ignores_macro_rules_inside_trivia_and_literals() {
    let source = r#"
const TEXT: &str = "macro_rules! fake { () => { 1 } }";
// macro_rules! comment { () => { 2 } }
macro_rules! real { () => { 3 }; }
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["real"]);
}

#[test]
fn parser_preserves_unicode_and_literal_delimiters() {
    let source = "fn привет() {}\nmacro_rules! m { () => { let c = '}'; // }\n c }; }\n";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(&source[defs[0].span.clone()], "macro_rules! m { () => { let c = '}'; // }\n c }; }");
}

#[test]
fn parser_supports_all_definition_and_transcriber_delimiters() {
    let source = "macro_rules! a (($x:expr) => [$x];);\nmacro_rules! b [($x:expr) => ($x);];";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].arms.len(), 1);
    assert_eq!(defs[1].arms.len(), 1);
}
~~~

- [ ] **Step 2: Run parser tests and verify RED**

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml parser_ -- --nocapture`

Expected: failures for false macro discovery, character/comment delimiters, and non-brace definition delimiters.

- [ ] **Step 3: Add the lexer dependency and token model**

Add:

~~~toml
ra-ap-rustc_lexer = "0.174.0"
~~~

Tokenize the complete document with `tokenize(source, FrontmatterAllowed::Yes)`. Accumulate each token's `u32` length into `usize` byte offsets and retain the exact `source[start..end]` slice. Treat `Whitespace`, `LineComment`, and `BlockComment` as trivia for grammar navigation; include comments in `significant_tokens` so the safety oracle detects comment movement or swallowed code.

- [ ] **Step 4: Replace macro scanning with typed delimiter stacks**

Implement:

~~~rust
fn lex(source: &str) -> anyhow::Result<Vec<SourceToken>>;
fn next_non_trivia(tokens: &[SourceToken], from: usize) -> Option<usize>;
fn matching_delimiter(tokens: &[SourceToken], open: usize) -> anyhow::Result<usize>;
fn scan_arms(
    source: &str,
    tokens: &[SourceToken],
    open: usize,
    close: usize,
) -> anyhow::Result<Vec<MacroArm>>;
~~~

Require exact matching pairs `()`, `[]`, `{}`. Locate `=>` as adjacent non-trivia `Eq` and `Gt` tokens. Use the complete transcriber delimiter range for `body_span`. Include the optional semicolon after `()`/`[]` definitions in `MacroDef.span`.

- [ ] **Step 5: Verify GREEN and existing parser compatibility**

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml parser_ -- --nocapture`

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml tests::test_macro_heavy_file -- --exact`

Expected: all selected tests pass and every returned range slices valid UTF-8.

---

### Task 2: Token-preservation oracle and explicit results

**Files:**
- Modify: `rust-fmt-mf/src/types.rs`
- Modify: `rust-fmt-mf/src/lib.rs`
- Modify: `rust-fmt-mf/src/main.rs`
- Modify: `rust-fmt-mf/tests/integration.rs`

**Interfaces:**
- Produces: `MacroStatus::{Formatted, Unchanged, Skipped { reason: String }}`
- Produces: `MacroOutcome { name: String, span: Range<usize>, status: MacroStatus }`
- Produces: `FormatResult { text: String, macros: Vec<MacroOutcome> }`
- Produces: `format_source_with_report(...) -> anyhow::Result<FormatResult>`
- Keeps: `format_source(...) -> anyhow::Result<String>`.

- [ ] **Step 1: Write failing safety regressions**

~~~rust
fn assert_tokens_preserved(input: &str, output: &str) {
    let before = rust_fmt_mf::parser::significant_tokens(input).unwrap();
    let after = rust_fmt_mf::parser::significant_tokens(output).unwrap();
    assert_eq!(
        before.iter().map(|t| (&t.kind, &t.text)).collect::<Vec<_>>(),
        after.iter().map(|t| (&t.kind, &t.text)).collect::<Vec<_>>()
    );
}

#[test]
fn formatting_preserves_literals_unicode_and_user_marker_names() {
    let source = r#"
fn привет() {}
macro_rules! m { ($x:expr) => { let __m_0 = "a . b  c & d :: e"; $x + __m_0 }; }
"#;
    let output = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &output);
}

#[test]
fn public_call_is_idempotent() {
    let source = include_str!("fixtures/huge_macro.rs");
    let once = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    let twice = rust_fmt_mf::format_source(&once, "rustfmt", "2021", None).unwrap();
    assert_eq!(once, twice);
}
~~~

- [ ] **Step 2: Run the corruption test and verify RED**

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml --test integration formatting_preserves_literals_unicode_and_user_marker_names -- --exact`

Expected: failure showing changed identifiers or string literal tokens.

- [ ] **Step 3: Add report types and a non-recursive safety gate**

Refactor the pipeline into:

~~~rust
fn format_source_once(...) -> anyhow::Result<FormatResult>;

pub fn format_source_with_report(...) -> anyhow::Result<FormatResult> {
    let first = format_source_once(source, rustfmt_path, edition, config_path)?;
    ensure_tokens_preserved(source, &first.text)?;
    let second = format_source_once(&first.text, rustfmt_path, edition, config_path)?;
    ensure_tokens_preserved(&first.text, &second.text)?;
    anyhow::ensure!(first.text == second.text, "macro formatting is not idempotent");
    Ok(first)
}
~~~

`format_source` returns `format_source_with_report(...).map(|result| result.text)`. Remove the four-pass convergence loop.

- [ ] **Step 4: Emit diagnostics without contaminating stdout**

Keep formatted Rust on stdout. Write one stderr record for each outcome:

~~~text
rust-fmt-mf	FORMATTED	<name>	<start>..<end>
rust-fmt-mf	UNCHANGED	<name>	<start>..<end>
rust-fmt-mf	SKIPPED	<name>	<start>..<end>	<reason>
~~~

On invariant failure, return non-zero before writing stdout.

- [ ] **Step 5: Verify the safety tests**

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml --test integration -- --nocapture`

Expected: new tests now fail only where Task 3 still has lossy mapper/replacer behavior.

---

### Task 3: Lossless markers, matchers, and arm fallback

**Files:**
- Modify: `rust-fmt-mf/src/types.rs`
- Modify: `rust-fmt-mf/src/replacer.rs`
- Modify: `rust-fmt-mf/src/mapper.rs`
- Modify: `rust-fmt-mf/src/shadow.rs`
- Modify: `rust-fmt-mf/src/lib.rs`
- Add fixtures under: `rust-fmt-mf/tests/fixtures/`

**Interfaces:**
- Produces: `Mapping::with_prefix(prefix: String) -> Mapping`
- Produces: deterministic `unique_prefix(source: &str) -> String`
- Produces: per-arm `ArmResult { replacement: Option<String>, status: MacroStatus }`.

- [ ] **Step 1: Add adversarial golden fixtures**

Create input/expected pairs for `matcher_line_comment`, `literal_delimiters`, `unicode_identifiers`, `marker_collision`, `macro_text_in_literal`, `all_delimiters`, `arbitrary_separator`, `tuple_trailing_comma`, and `opaque_dsl`.

The matcher-comment input contains:

~~~rust
macro_rules! commented_matcher {
    (
        $left:expr, // left operand
        $right:expr $(,)?
    ) => {{ $left + $right }};
}
~~~

Every expected file preserves the exact significant token sequence. The opaque fixture preserves its transcriber bytes and expects a `SKIPPED` diagnostic.

- [ ] **Step 2: Build and verify RED on the new fixtures**

Run: `cargo build --manifest-path rust-fmt-mf/Cargo.toml --bin rust-fmt-mf`

Run each new fixture through `rust-fmt-mf/target/debug/rust-fmt-mf` and compare stdout to its sibling `.expected`.

Expected: matcher comment, Unicode, literals, and marker collision fail before implementation.

- [ ] **Step 3: Make every synthetic name collision-free**

Implement:

~~~rust
fn unique_prefix(source: &str) -> String {
    (0..)
        .map(|n| format!("__rust_fmt_mf_{n}_"))
        .find(|candidate| !source.contains(candidate))
        .unwrap()
}
~~~

Use the prefix for metavariable identifiers, repetition wrappers, shadow arms, and final-pass markers. Restore only registered placeholders and require each registered marker to occur exactly once in its shadow section.

- [ ] **Step 4: Remove raw-text normalization from significant regions**

Delete active global `.replace()` calls over restored Rust. Normalize matcher whitespace only in lexer gaps. Preserve a mandatory newline after every line comment. Before collapsing any other gap, re-tokenize the neighboring source and require the same two significant tokens.

Never add or remove punctuation. Remove the trailing-comma deletion in `collapse_simple_delimited`.

- [ ] **Step 5: Format each arm independently and skip safely**

Build and run one shadow wrapper per arm. When stable `rustfmt` rejects an arm, retain the exact original `body_span` and set:

~~~rust
struct ArmResult {
    replacement: Option<String>,
    status: MacroStatus,
}
~~~

`None` copies original bytes. A macro is `Skipped` when any arm is skipped, `Formatted` when output differs only in trivia, otherwise `Unchanged`. Sibling arms continue formatting.

- [ ] **Step 6: Verify new and existing fixtures**

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml --all-targets`

Run: `python3 rust-fmt-mf/tests/run_fixtures.py --binary rust-fmt-mf/target/debug/rust-fmt-mf`

Expected: token-preservation and matcher-comment regressions pass. Any layout change is an explicit golden diff, never a token change.

---

### Task 4: Safe VS Code native boundary

**Files:**
- Modify: `src/formatter.ts`
- Modify: `src/test/suite/formatter.test.ts`

**Interfaces:**
- Keeps: `formatWithNativeMacroFormatter(...) -> Promise<string | null>`
- Produces: `sha256File(path: string) -> Promise<string>`
- Removes from active formatting: `normalizeMacroSpacing` and `normalizeMacroBodies`.

- [ ] **Step 1: Write failing TypeScript boundary tests**

Exercise the real `RustFormatter.formatWithContext` path with the bundled binary:

~~~ts
test('preserves literal bytes through the complete native path', async () => {
    const formatter = new RustFormatter({
        rustfmtPath: 'rustfmt',
        extraArgs: [],
        nativeMacroFormatter: true
    });
    const input = 'const S: &str = "a  b";\nmacro_rules! m { () => { S }; }\n';
    const result = await formatter.formatWithContext(input, { cwd: undefined, edition: '2021' });
    assert.ok(result);
    assert.ok(result!.includes('"a  b"'));
});
~~~

- [ ] **Step 2: Verify RED**

Run: `npm run compile`

Run: `npm test -- --grep "preserves literal bytes"`

Expected: tests fail because the active path uses the heuristic normalizers.

- [ ] **Step 3: Pass original text through both paths**

Call the native formatter with `text`. If it returns `null`, pass the same `text` to ordinary `rustfmt`. Remove the heuristic formatter from the active flow and delete it if no production caller remains.

Log successful native stderr diagnostics. Compute SHA-256 with Node's built-in `crypto.createHash('sha256')` and log `<path> sha256=<digest>` once per binary path.

- [ ] **Step 4: Verify TypeScript**

Run: `npm run lint`

Run: `npm run compile`

Run: `npm test`

Expected: all commands pass with no new npm dependency.

---

### Task 5: Complete acceptance and coverage runner

**Files:**
- Modify: `rust-fmt-mf/tests/run_fixtures.py`
- Modify: `rust-fmt-mf/tests/test_run_fixtures.py`

**Interfaces:**
- CLI: `run_fixtures.py --binary PATH [--rustfmt PATH] [--corpus PATH] [--cargo-manifest PATH]`
- Default corpus: repository `test-rs/src`
- Produces: safety, golden, formatted, unchanged, and skipped totals.

- [ ] **Step 1: Write failing discovery and classification tests**

~~~python
def test_discovers_every_fixture(self):
    cases = run_fixtures.discover_fixtures(self.fixture_dir)
    self.assertEqual([case.name for case in cases], ["a", "b"])

def test_missing_expected_is_reported(self):
    with self.assertRaisesRegex(ValueError, "missing expected"):
        run_fixtures.discover_fixtures(self.fixture_dir)

def test_status_parser_counts_skipped(self):
    report = run_fixtures.parse_diagnostics(
        "rust-fmt-mf\\tSKIPPED\\tm\\t0..10\\topaque DSL\\n"
    )
    self.assertEqual(report.skipped, 1)
~~~

- [ ] **Step 2: Verify RED**

Run: `python3 -m unittest discover -s rust-fmt-mf/tests -p 'test_*.py' -v`

Expected: missing discovery and diagnostic APIs.

- [ ] **Step 3: Replace the manual fixture list**

Discover sorted `*.rs` inputs with `Path.glob`. Require one sibling `.expected` per input and reject orphan expected files. Keep diagnostics inside `rust-fmt-mf/target/macro-audit`.

- [ ] **Step 4: Add corpus and coverage checks**

Use native stderr for macro outcomes. Recursively audit every `*.rs` below the corpus path, run the binary twice in memory, and validate the first output with stable `rustfmt`. Token comparison remains in the native engine so Python does not implement another Rust lexer.

When `--cargo-manifest` is supplied, copy that crate under `rust-fmt-mf/target/macro-audit/corpus-workspace` with `shutil.copytree`, excluding `target` and `.git`. Replace only the copied Rust files with their audited first-pass outputs and run `cargo check --all-targets --manifest-path <copied Cargo.toml>`. A compile failure is `SYNTAX_ERROR` and retains the copied workspace as evidence.

Print:

~~~text
SUMMARY safety=<passed>/<all> golden=<passed>/<all> formatted=<n> unchanged=<n> skipped=<n>
~~~

All denominators come from discovered cases and diagnostics.

- [ ] **Step 5: Verify the complete corpus**

Run: `python3 -m unittest discover -s rust-fmt-mf/tests -p 'test_*.py' -v`

Run: `python3 rust-fmt-mf/tests/run_fixtures.py --binary rust-fmt-mf/target/debug/rust-fmt-mf --corpus test-rs/src --cargo-manifest test-rs/Cargo.toml`

Expected: every checked-in fixture and every corpus file is listed; safety and golden coverage are 100%; skipped count is explicit.

---

### Task 6: Bundled artifact equality and cross-platform CI

**Files:**
- Modify: `scripts/build_current.py`
- Modify: `scripts/test_build_current.py`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: `sha256(path: Path) -> str`
- Build invariant: copied bundle hash equals the release artifact hash.

- [ ] **Step 1: Write a failing hash test**

~~~python
def test_sha256_is_stable(self):
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "binary"
        path.write_bytes(b"formatter")
        self.assertEqual(sha256(path), hashlib.sha256(b"formatter").hexdigest())
~~~

Add `hashlib` and `tempfile` to the test imports and import `sha256` from `scripts.build_current`.

- [ ] **Step 2: Verify RED**

Run: `python3 -m unittest scripts/test_build_current.py -v`

Expected: import failure for `sha256`.

- [ ] **Step 3: Verify artifact equality after copying**

Implement streaming SHA-256 with `hashlib`. After `shutil.copy2`, compare source and destination digests, fail on mismatch, and print the verified destination digest.

- [ ] **Step 4: Extend CI acceptance**

Keep the Ubuntu/Windows native matrix. Run Python unit tests before building, then run the complete fixture/corpus command against the matrix binary. On Linux assert only the Linux executable bit; on Windows use the `.exe` path. Upload `target/macro-audit` only on failure.

- [ ] **Step 5: Verify script tests**

Run: `python3 -m unittest scripts/test_build_current.py -v`

Run: `python3 -m unittest discover -s rust-fmt-mf/tests -p 'test_*.py' -v`

Expected: all unit tests pass.

---

### Task 7: Refresh Linux bundle and final verification

**Files:**
- Modify on Linux: `bin/linux-x64/rust-fmt-mf`
- Preserve: `bin/win32-x64/rust-fmt-mf.exe`
- Verify: source, tests, fixtures, docs, and user `test-rs` edits.

**Interfaces:**
- Produces: bundled Linux binary built from final current source.

- [ ] **Step 1: Record the Windows binary hash**

Run: `sha256sum bin/win32-x64/rust-fmt-mf.exe`

Do not modify the Windows artifact on Linux.

- [ ] **Step 2: Run all source checks**

Run: `cargo fmt --manifest-path rust-fmt-mf/Cargo.toml --all -- --check`

Run: `cargo test --manifest-path rust-fmt-mf/Cargo.toml --all-targets`

Run: `python3 -m unittest discover -s rust-fmt-mf/tests -p 'test_*.py' -v`

Run: `npm run lint`

Run: `npm run compile`

Expected: every command exits zero.

- [ ] **Step 3: Build and copy Linux release binary**

Run: `python3 scripts/build_current.py --release`

Expected: identical source/destination SHA-256 and an executable `bin/linux-x64/rust-fmt-mf`.

- [ ] **Step 4: Run bundled-binary acceptance**

Run: `python3 rust-fmt-mf/tests/run_fixtures.py --binary bin/linux-x64/rust-fmt-mf --corpus test-rs/src --cargo-manifest test-rs/Cargo.toml`

Expected: safety and golden coverage are 100%; every macro outcome is counted; cargo check succeeds.

- [ ] **Step 5: Prove artifact identity and preservation**

Run:

~~~bash
cmp rust-fmt-mf/target/release/rust-fmt-mf bin/linux-x64/rust-fmt-mf
test -x bin/linux-x64/rust-fmt-mf
sha256sum bin/win32-x64/rust-fmt-mf.exe
git diff --check
git status --short
~~~

Expected: release and bundled Linux binaries match, Windows hash equals Step 1, no commits were created, user `test-rs` changes remain, and only intended files changed.
