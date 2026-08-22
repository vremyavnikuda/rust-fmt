# Batch Macro Definition Shadow Formatting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the number of `rustfmt` subprocess spawns needed to format a macro-heavy file by batching all `macro_rules!` definitions in a file into one combined shadow-format call per pass instead of one call per definition, with zero change to formatted output.

**Architecture:** `format_source_once` currently loops over every macro definition and calls `format_definition_once` once per definition (one `rustfmt` shadow-file call each — measured at 21 of 34 shadow-mode calls per pass, 62%, on a 21-macro fixture). The shadow-file/apply-formatting machinery (`build_shadow_file_from_strings`, `split_shadow_into_arms`, `apply_formatting`) already operates on flat, positionally-ordered lists spanning an arbitrary number of definitions — it is just never called with more than one definition's arms at a time today. This plan adds `format_definitions_batch`, which feeds every deep-formattable definition's arms into one combined shadow file and one `run_rustfmt` call, applies the result, and falls back to the existing one-call-per-definition loop only if the batch fails the token-preservation check (rare — 100% of the current fixture/corpus passes today).

**Tech Stack:** Rust (`rust-fmt-mf` crate), `anyhow`, `std::process::Command`, `std::sync::atomic`.

**Spec:** `docs/superpowers/specs/2026-08-22-batch-macro-shadow-formatting-design.md`

## Global Constraints

- Output must be byte-identical to today's output for every existing fixture and corpus file — this is a performance change only, never a formatting-behavior change.
- No new public CLI flags, VS Code settings, or commands.
- `preformat_rep_bodies`, `format_macro_invocations`, and `run_rustfmt_no_macro` are out of scope — do not touch them.
- `format_definition_once`'s observable behavior (output for any given input) must stay byte-identical and it must be reused as-is in the fallback path. Its internal body-extraction logic must not be duplicated in the new batch function — both call one shared helper (`build_arm_shadow_fragment`, introduced in Task 4).

---

### Task 1: Add rustfmt subprocess call-count instrumentation

**Files:**
- Modify: `rust-fmt-mf/src/formatter.rs`
- Test: `rust-fmt-mf/src/tests.rs`

**Interfaces:**
- Produces: `pub fn rustfmt_call_count() -> usize` and `pub fn reset_rustfmt_call_count()` in `crate::formatter`, usable from both `tests.rs` (via `crate::formatter::...`) and `integration.rs` (via `rust_fmt_mf::formatter::...`). Both `run_rustfmt` and `run_rustfmt_no_macro` increment the counter once per successful `Command::spawn()`.

- [ ] **Step 1: Write the failing test**

Add to `rust-fmt-mf/src/tests.rs` (append at end of file):

```rust
#[test]
fn rustfmt_call_count_tracks_successful_spawns() {
    crate::formatter::reset_rustfmt_call_count();
    assert_eq!(crate::formatter::rustfmt_call_count(), 0);
    crate::formatter::run_rustfmt("fn main() {}", "rustfmt", "2021", None).unwrap();
    assert_eq!(crate::formatter::rustfmt_call_count(), 1);
    crate::formatter::run_rustfmt_no_macro("fn main() {}", "rustfmt", "2021", None).unwrap();
    assert_eq!(crate::formatter::rustfmt_call_count(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd rust-fmt-mf && cargo test --lib rustfmt_call_count_tracks_successful_spawns`
Expected: FAIL with a compile error — `rustfmt_call_count`/`reset_rustfmt_call_count` do not exist yet.

- [ ] **Step 3: Implement the counter**

Replace the top of `rust-fmt-mf/src/formatter.rs` (the `use` lines) with:

```rust
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static RUSTFMT_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Number of times `run_rustfmt`/`run_rustfmt_no_macro` have successfully
/// spawned a `rustfmt` process since the last `reset_rustfmt_call_count()`.
/// Test-only instrumentation for asserting on subprocess-spawn counts
/// instead of flaky wall-clock timing.
pub fn rustfmt_call_count() -> usize {
    RUSTFMT_CALL_COUNT.load(Ordering::SeqCst)
}

pub fn reset_rustfmt_call_count() {
    RUSTFMT_CALL_COUNT.store(0, Ordering::SeqCst);
}
```

Then, in `run_rustfmt_no_macro`, right after `let mut child = cmd.spawn()?;` add one line:

```rust
    let mut child = cmd.spawn()?;
    RUSTFMT_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
```

And in `run_rustfmt`, right after its own `let mut child = cmd.spawn()?;` add the same line:

```rust
    let mut child = cmd.spawn()?;
    RUSTFMT_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd rust-fmt-mf && cargo test --lib rustfmt_call_count_tracks_successful_spawns`
Expected: PASS

- [ ] **Step 5: Run the full existing suite to confirm nothing else broke**

Run: `cd rust-fmt-mf && cargo test --release`
Expected: all previously-passing tests still pass (91 total before this task's new test).

- [ ] **Step 6: Commit**

```bash
cd rust-fmt-mf
git add src/formatter.rs src/tests.rs
git commit -m "feat(rust-fmt-mf): add rustfmt subprocess call-count instrumentation"
```

---

### Task 2: Extract and unit-test the batch/fallback decision helper

**Files:**
- Modify: `rust-fmt-mf/src/lib.rs`
- Test: `rust-fmt-mf/src/tests.rs`

**Interfaces:**
- Consumes: nothing new (uses the existing crate-private `ensure_tokens_preserved` already defined in `lib.rs`).
- Produces: `fn accepted_batch_result(original: &str, batch_result: anyhow::Result<String>) -> Option<String>` (crate-private, in `lib.rs`) — later tasks call this to decide whether a batched shadow-format result is safe to apply or whether the caller must fall back to per-definition formatting.

- [ ] **Step 1: Write the failing tests**

Add to `rust-fmt-mf/src/tests.rs` (append at end of file):

```rust
#[test]
fn accepted_batch_result_uses_candidate_when_tokens_preserved() {
    let original = "fn main() { 1 + 1; }";
    let candidate = "fn main() {\n    1 + 1;\n}".to_string();
    let result = super::accepted_batch_result(original, Ok(candidate.clone()));
    assert_eq!(result, Some(candidate));
}

#[test]
fn accepted_batch_result_falls_back_when_tokens_change() {
    let original = "fn main() { 1 + 1; }";
    let corrupted = "fn main() { 1 + 2; }".to_string();
    let result = super::accepted_batch_result(original, Ok(corrupted));
    assert_eq!(result, None);
}

#[test]
fn accepted_batch_result_falls_back_on_error() {
    let original = "fn main() { 1 + 1; }";
    let result = super::accepted_batch_result(original, Err(anyhow::anyhow!("boom")));
    assert_eq!(result, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd rust-fmt-mf && cargo test --lib accepted_batch_result`
Expected: FAIL with a compile error — `accepted_batch_result` does not exist yet.

- [ ] **Step 3: Implement the helper**

In `rust-fmt-mf/src/lib.rs`, add this function immediately above `fn format_definition_once(` (around line 1004):

```rust
/// Decide whether a batched shadow-format result is safe to apply. Returns
/// `Some(candidate)` when the batch produced output that exactly preserves
/// the original's significant tokens; `None` means the caller must fall
/// back to formatting definitions one at a time.
fn accepted_batch_result(original: &str, batch_result: anyhow::Result<String>) -> Option<String> {
    match batch_result {
        Ok(candidate) if ensure_tokens_preserved(original, &candidate).is_ok() => Some(candidate),
        _ => None,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd rust-fmt-mf && cargo test --lib accepted_batch_result`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
cd rust-fmt-mf
git add src/lib.rs src/tests.rs
git commit -m "feat(rust-fmt-mf): extract batch/fallback acceptance decision as a pure function"
```

---

### Task 3: Write the failing perf regression test

**Files:**
- Test: `rust-fmt-mf/src/tests.rs`

**Interfaces:**
- Consumes: `super::format_source` (already `pub`, existing), `crate::formatter::reset_rustfmt_call_count` / `rustfmt_call_count` (from Task 1).

- [ ] **Step 1: Write the failing test**

Add to `rust-fmt-mf/src/tests.rs` (append at end of file):

```rust
#[test]
fn formatting_many_independent_macros_uses_few_rustfmt_calls() {
    let source = r#"macro_rules! one {
    ($x:expr) => {
        $x + 1
    };
}

macro_rules! two {
    ($x:expr) => {
        $x + 2
    };
}

macro_rules! three {
    ($x:expr) => {
        $x + 3
    };
}

macro_rules! four {
    ($x:expr) => {
        $x + 4
    };
}

macro_rules! five {
    ($x:expr) => {
        $x + 5
    };
}
"#;
    crate::formatter::reset_rustfmt_call_count();
    let _ = super::format_source(source, "rustfmt", "2021", None).unwrap();
    let calls = crate::formatter::rustfmt_call_count();
    // Today this needs one rustfmt call per definition per convergence pass
    // (5 definitions x >=2 passes = 10+). Batched, it should need roughly
    // one shadow call plus one final-pass call per convergence pass. This
    // must NOT scale with the number of definitions.
    assert!(
        calls <= 8,
        "expected batched formatting of 5 independent macros to use at most 8 rustfmt calls, used {calls}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd rust-fmt-mf && cargo test --lib formatting_many_independent_macros_uses_few_rustfmt_calls -- --nocapture`
Expected: FAIL — the assertion fails because today's implementation uses one call per definition per pass (well above 8). Record the actual failing count printed in the assertion message; it will be used in Task 4 Step 4 to confirm the improvement.

- [ ] **Step 3: Commit the failing test as a checkpoint**

```bash
cd rust-fmt-mf
git add src/tests.rs
git commit -m "test(rust-fmt-mf): add failing perf regression test for batched macro formatting"
```

---

### Task 4: Implement batched definition formatting in `format_source_once`

**Files:**
- Modify: `rust-fmt-mf/src/lib.rs:1004-1087` (both `format_definition_once` and `format_source_once`)

**Interfaces:**
- Consumes: `accepted_batch_result` (Task 2), `crate::formatter::run_rustfmt` (existing), `build_shadow_file_from_strings` (existing, unchanged), `apply_formatting` (existing, unchanged).
- Produces:
  - `fn build_arm_shadow_fragment(source: &str, arm: &crate::types::MacroArm, marker_prefix: &str, rustfmt_path: &str, edition: &str, config_path: Option<&str>) -> (String, Mapping)` (crate-private, new) — the single place that extracts, normalizes, and pre-formats one arm's body. Both `format_definition_once` and the new `format_definitions_batch` call this instead of inlining the extraction logic, so there is exactly one copy of it.
  - `fn format_definitions_batch(source: &str, definitions: &[&crate::types::MacroDef], rustfmt_path: &str, edition: &str, config_path: Option<&str>) -> anyhow::Result<String>` (crate-private, new).
- `format_definition_once`'s signature and return value for any given input are unchanged — only its body is refactored to call the new shared helper instead of inlining the same code.

- [ ] **Step 1: Add `build_arm_shadow_fragment` and refactor `format_definition_once` to use it**

In `rust-fmt-mf/src/lib.rs`, add this function immediately above `fn format_definition_once(` (around line 1004):

```rust
/// Extract, normalize, and pre-format one macro arm's shadow-file body
/// fragment. Shared by `format_definition_once` (single-definition path)
/// and `format_definitions_batch` (multi-definition path) so the two never
/// drift out of sync with each other.
fn build_arm_shadow_fragment(
    source: &str,
    arm: &crate::types::MacroArm,
    marker_prefix: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> (String, Mapping) {
    let arm_body_text = &source[arm.body_span.clone()];
    let body_text = arm_body_text.trim();
    let inner = if body_text.starts_with("{{") && body_text.ends_with("}}") {
        &body_text[2..body_text.len() - 2]
    } else {
        &body_text[1..body_text.len() - 1]
    };
    let trimmed = inner.strip_prefix('\n').unwrap_or(inner);
    let inner_text = trimmed.strip_suffix('\n').unwrap_or(trimmed);
    let mut mapping = Mapping::with_prefix(marker_prefix.to_string());
    let mut inner_str = normalize_body_indent(inner_text);
    inner_str = replace_macro_syntax_text(&inner_str, &mut mapping);
    inner_str = preformat_rep_bodies(
        &inner_str,
        &format!("{marker_prefix}rep_"),
        rustfmt_path,
        edition,
        config_path,
    );
    (inner_str, mapping)
}
```

Then replace the existing `format_definition_once` (`rust-fmt-mf/src/lib.rs:1004-1048`):

```rust
fn format_definition_once(
    source: &str,
    definition: &crate::types::MacroDef,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    if !supports_deep_format(source, definition) {
        return Ok(format_definition_without_brace_bodies(source, definition));
    }
    let mut all_replaced_bodies_str: Vec<String> = Vec::new();
    let mut all_mappings: Vec<Mapping> = Vec::new();
    let marker_prefix = unique_prefix(source);
    for arm in &definition.arms {
        let arm_body_text = &source[arm.body_span.clone()];
        let body_text = arm_body_text.trim();
        let inner = if body_text.starts_with("{{") && body_text.ends_with("}}") {
            &body_text[2..body_text.len() - 2]
        } else {
            &body_text[1..body_text.len() - 1]
        };
        let trimmed = inner.strip_prefix('\n').unwrap_or(inner);
        let inner_text = trimmed.strip_suffix('\n').unwrap_or(trimmed);
        let mut mapping = Mapping::with_prefix(marker_prefix.clone());
        let mut inner_str = normalize_body_indent(inner_text);
        inner_str = replace_macro_syntax_text(&inner_str, &mut mapping);
        inner_str = preformat_rep_bodies(
            &inner_str,
            &format!("{marker_prefix}rep_"),
            rustfmt_path,
            edition,
            config_path,
        );
        all_replaced_bodies_str.push(inner_str);
        all_mappings.push(mapping);
    }
    let shadow_code = build_shadow_file_from_strings(&all_replaced_bodies_str, &marker_prefix);
    let formatted_shadow = run_rustfmt(&shadow_code, rustfmt_path, edition, config_path)?;
    Ok(apply_formatting(
        source,
        std::slice::from_ref(definition),
        &formatted_shadow,
        &all_mappings,
    ))
}
```

with:

```rust
fn format_definition_once(
    source: &str,
    definition: &crate::types::MacroDef,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    if !supports_deep_format(source, definition) {
        return Ok(format_definition_without_brace_bodies(source, definition));
    }
    let mut all_replaced_bodies_str: Vec<String> = Vec::new();
    let mut all_mappings: Vec<Mapping> = Vec::new();
    let marker_prefix = unique_prefix(source);
    for arm in &definition.arms {
        let (inner_str, mapping) = build_arm_shadow_fragment(
            source,
            arm,
            &marker_prefix,
            rustfmt_path,
            edition,
            config_path,
        );
        all_replaced_bodies_str.push(inner_str);
        all_mappings.push(mapping);
    }
    let shadow_code = build_shadow_file_from_strings(&all_replaced_bodies_str, &marker_prefix);
    let formatted_shadow = run_rustfmt(&shadow_code, rustfmt_path, edition, config_path)?;
    Ok(apply_formatting(
        source,
        std::slice::from_ref(definition),
        &formatted_shadow,
        &all_mappings,
    ))
}
```

This step must not change `format_definition_once`'s behavior at all — it is a pure extract-method refactor. Verify with: `cd rust-fmt-mf && cargo test --release` (all 91 existing tests must still pass, unchanged) before moving to Step 2.

- [ ] **Step 2: Add `format_definitions_batch`**

In `rust-fmt-mf/src/lib.rs`, add this function immediately above `fn format_source_once(` (around line 1057), right after `accepted_batch_result` from Task 2:

```rust
/// Format every arm of every given definition in one combined shadow file
/// and one `rustfmt` call, instead of one call per definition. Definitions
/// must already be filtered to `supports_deep_format` and given in
/// ascending source-position order (the natural order `parse_macro_defs`
/// returns).
fn format_definitions_batch(
    source: &str,
    definitions: &[&crate::types::MacroDef],
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let marker_prefix = unique_prefix(source);
    let mut all_replaced_bodies_str: Vec<String> = Vec::new();
    let mut all_mappings: Vec<Mapping> = Vec::new();
    for definition in definitions {
        for arm in &definition.arms {
            let (inner_str, mapping) = build_arm_shadow_fragment(
                source,
                arm,
                &marker_prefix,
                rustfmt_path,
                edition,
                config_path,
            );
            all_replaced_bodies_str.push(inner_str);
            all_mappings.push(mapping);
        }
    }
    let shadow_code = build_shadow_file_from_strings(&all_replaced_bodies_str, &marker_prefix);
    let formatted_shadow = run_rustfmt(&shadow_code, rustfmt_path, edition, config_path)?;
    let owned_definitions: Vec<crate::types::MacroDef> = definitions
        .iter()
        .map(|definition| (*definition).clone())
        .collect();
    Ok(apply_formatting(
        source,
        &owned_definitions,
        &formatted_shadow,
        &all_mappings,
    ))
}
```

- [ ] **Step 3: Replace `format_source_once`'s body**

Replace the entire current `format_source_once` function (`rust-fmt-mf/src/lib.rs:1057-1087`):

```rust
fn format_source_once(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<OnceResult> {
    let definitions = parse_macro_defs(source)?;
    let mut text = source.to_string();
    let mut skipped_reasons = vec![None; definitions.len()];

    for (index, definition) in definitions.iter().enumerate().rev() {
        match format_definition_once(&text, definition, rustfmt_path, edition, config_path) {
            Ok(candidate) => match ensure_tokens_preserved(&text, &candidate) {
                Ok(()) => text = candidate,
                Err(error) => {
                    skipped_reasons[index] = Some(format!("lossless check failed: {error}"));
                }
            },
            Err(error) => {
                skipped_reasons[index] = Some(format!("shadow formatting failed: {error}"));
            }
        }
    }

    let formatted = final_format_pass(&text, rustfmt_path, edition, config_path)?;
    ensure_tokens_preserved(&text, &formatted)?;
    Ok(OnceResult {
        text: formatted,
        skipped_reasons,
    })
}
```

with:

```rust
fn format_source_once(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<OnceResult> {
    let definitions = parse_macro_defs(source)?;
    let mut text = source.to_string();
    let mut skipped_reasons = vec![None; definitions.len()];

    // Phase 1: definitions that cannot be deep-formatted get a pure string
    // transform only (no rustfmt call). Reverse order keeps byte offsets of
    // not-yet-processed definitions valid while `text` is being edited.
    for (index, definition) in definitions.iter().enumerate().rev() {
        if supports_deep_format(source, definition) {
            continue;
        }
        let candidate = format_definition_without_brace_bodies(&text, definition);
        match ensure_tokens_preserved(&text, &candidate) {
            Ok(()) => text = candidate,
            Err(error) => {
                skipped_reasons[index] = Some(format!("lossless check failed: {error}"));
            }
        }
    }

    // Phase 2: deep-formattable definitions. Re-parse since phase 1 may have
    // shifted byte offsets; phase 1 never adds, removes, or reorders
    // definitions, so index i in `definitions` still matches index i here.
    let refreshed_definitions = parse_macro_defs(&text)?;
    let deep_definitions: Vec<(usize, &crate::types::MacroDef)> = refreshed_definitions
        .iter()
        .enumerate()
        .filter(|(_, definition)| supports_deep_format(&text, definition))
        .collect();

    if !deep_definitions.is_empty() {
        let just_defs: Vec<&crate::types::MacroDef> = deep_definitions
            .iter()
            .map(|(_, definition)| *definition)
            .collect();
        let batch_result =
            format_definitions_batch(&text, &just_defs, rustfmt_path, edition, config_path);
        match accepted_batch_result(&text, batch_result) {
            Some(candidate) => text = candidate,
            None => {
                // Fall back to the proven one-call-per-definition path so a
                // single problematic definition among many healthy ones is
                // still isolated and reported SKIPPED individually.
                for &(index, definition) in deep_definitions.iter().rev() {
                    match format_definition_once(&text, definition, rustfmt_path, edition, config_path) {
                        Ok(candidate) => match ensure_tokens_preserved(&text, &candidate) {
                            Ok(()) => text = candidate,
                            Err(error) => {
                                skipped_reasons[index] =
                                    Some(format!("lossless check failed: {error}"));
                            }
                        },
                        Err(error) => {
                            skipped_reasons[index] =
                                Some(format!("shadow formatting failed: {error}"));
                        }
                    }
                }
            }
        }
    }

    let formatted = final_format_pass(&text, rustfmt_path, edition, config_path)?;
    ensure_tokens_preserved(&text, &formatted)?;
    Ok(OnceResult {
        text: formatted,
        skipped_reasons,
    })
}
```

- [ ] **Step 4: Build and fix any compile errors**

Run: `cd rust-fmt-mf && cargo build --release 2>&1 | tail -60`
Expected: clean build. If the borrow checker rejects any part of the above (e.g. a lifetime issue between `deep_definitions` borrowing `refreshed_definitions` and the later `text = candidate` reassignment), the fix is almost always to shrink the borrow's scope — e.g. compute `just_defs` and drop `deep_definitions`'s borrow before mutating `text` in the `Some` arm. Do not change the two-phase design or the fallback behavior to work around a borrow error.

- [ ] **Step 5: Run the Task 3 perf test and record the improvement**

Run: `cd rust-fmt-mf && cargo test --lib formatting_many_independent_macros_uses_few_rustfmt_calls -- --nocapture`
Expected: PASS. If it still fails, read the printed call count — if it's just over 8 because convergence needs one more pass than assumed, that's fine: update the `8` in the Task 3 test to the actual observed value plus 2 (safety margin for one extra convergence pass), but the count must still be small and constant, not scaling with the 5 definitions in the fixture.

- [ ] **Step 6: Run the full existing regression suite — must be unchanged**

Run: `cd rust-fmt-mf && cargo test --release`
Expected: every test that passed before this task still passes, with byte-identical assertions unchanged (in particular `real_macro_heavy_matches_user_golden`, `real_macro_edge_cases_match_golden`, `real_macro_missing_cases_match_golden`, `real_main_fmt_matches_golden`, and both `marker_collision_is_idempotent`-style idempotence tests).

- [ ] **Step 7: Run the python corpus audit — must stay at 100% conformance**

Run: `cd rust-fmt-mf && python3 tests/run_fixtures.py`
Expected:
```
Execution safety: 80/80 (100.0%)
Exact-output conformance: 75/75 (100.0%)
Macros handled without skip: 200/200 (100.0%)
```
If `Exact-output conformance` or `Macros handled without skip` drops even slightly, the batching changed real output — stop and investigate before proceeding; do not adjust golden fixtures to match new output for this task.

- [ ] **Step 8: Commit**

```bash
cd rust-fmt-mf
git add src/lib.rs src/tests.rs
git commit -m "feat(rust-fmt-mf): batch all macro definitions into one shadow-format call per pass"
```

---

### Task 5: Measure the real-world improvement and record it

**Files:**
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: `crate::formatter::rustfmt_call_count` / `reset_rustfmt_call_count` (Task 1), the release binary built from Task 4's code.

- [ ] **Step 1: Rebuild the release binary**

Run: `cd rust-fmt-mf && cargo build --release`

- [ ] **Step 2: Re-run the same measurement as the spec's Evidence section**

Run:
```bash
cd rust-fmt-mf
for i in 1 2 3; do
  /usr/bin/time -f "%e sec" ./target/release/rust-fmt-mf --edition 2021 < ../test-rs/src/examples/macro_heavy.rs > /dev/null 2>> /tmp/timing_after.txt
done
cat /tmp/timing_after.txt
strace -f -e trace=execve -o /tmp/strace_after.log ./target/release/rust-fmt-mf --edition 2021 < ../test-rs/src/examples/macro_heavy.rs > /dev/null 2>/dev/null
grep 'execve.*rustfmt' /tmp/strace_after.log | grep -v ENOENT | wc -l
```
Record the new wall-clock times and the new total successful `rustfmt` execve count, to compare against the spec's baseline (~1.0s, 94 calls).

- [ ] **Step 3: Add a CHANGELOG entry**

In `CHANGELOG.md`, under the `## Unreleased` section (create one directly below the `# Changelog` header block if it does not already exist, following the file's existing `## 0.1.8` entry format), add under a `### Changed` heading:

```markdown
- Macro definitions in the same file are now formatted in one combined `rustfmt` call per convergence pass instead of one call per definition, falling back to the previous per-definition behavior only if the batch fails the token-preservation check. Measured on a 21-macro fixture: `rustfmt` subprocess spawns dropped from 94 to <ACTUAL_MEASURED_COUNT>, wall-clock time from ~1.0s to <ACTUAL_MEASURED_TIME>.
```

Replace `<ACTUAL_MEASURED_COUNT>` and `<ACTUAL_MEASURED_TIME>` with the real numbers from Step 2 — do not leave the placeholders in the committed file.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md
git commit -m "chore: record macro-formatting batching performance improvement"
```
