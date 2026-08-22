# Batch Macro Definition Shadow Formatting Design

## Goal

Cut the number of `rustfmt` subprocess spawns needed to format a macro-heavy file, without changing a single byte of output. Every existing golden fixture, integration test, and the python corpus audit must remain byte-identical after this change. This is a pure internal performance change — no new settings, no new commands, no new UI.

## Evidence

Measured directly (not estimated) on `test-rs/src/examples/macro_heavy.rs` — 371 lines, 21 `macro_rules!` definitions, already fully formatted (every macro reports `UNCHANGED`):

- **Wall time:** ~1.0s per format (`/usr/bin/time`, 3 runs: 1.04s / 0.96s / 0.96s), for a file that changes nothing. This is the common case (re-saving an already-formatted file) and it is roughly 10-100x slower than what an editor format-on-save should feel like.
- **`rustfmt` subprocess count:** 94 successful spawns for one `format_source_with_report` call (via `strace -f -e trace=execve`), split by mode:
  - 90 spawns with `--config format_macro_bodies=true` (the "shadow file" trick, i.e. `run_rustfmt`)
  - 4 spawns with `--config format_macro_bodies=false` (`run_rustfmt_no_macro`, one per convergence pass plus final validation)
- **Per-pass breakdown of the 90 shadow-mode calls** (measured via temporary call-site instrumentation, one full pass = 34 calls):
  - **21 calls (62%)** — one `run_rustfmt` per `macro_rules!` definition, from `format_definition_once`'s shadow-file call (`lib.rs:1041`)
  - 12 calls (35%) — one `try_format_as_mod`/`try_format_as_fn` pair per `$()` repetition body, from `preformat_rep_bodies` (`lib.rs:146-148`)
  - 1 call (3%) — item/block-shaped macro invocation formatting, from `format_invocation_inner` (`lib.rs:512-513`)
- 34 calls/pass × ~2.6 passes to reach the fixed point ≈ the observed 90.

The dominant cost is the **one-`rustfmt`-call-per-macro-definition** pattern. Definitions already batch their own arms into one shadow file (`build_shadow_file_from_strings` already accepts multiple body strings) — the missing piece is batching *across* definitions in the same file.

## Scope

### Included
- Batch all `macro_rules!` definitions in a file that support deep formatting (`supports_deep_format`) into **one** combined shadow file and **one** `run_rustfmt` call per convergence pass, instead of one call per definition.
- A safe fallback: if the batched result fails the existing `ensure_tokens_preserved` check, fall back to the current one-call-per-definition sequential path for that pass, so per-definition `SKIPPED` isolation behaves exactly as it does today.
- A permanent, deterministic (non-timing-based) regression test asserting the `rustfmt` call count for a known multi-macro fixture stays low.
- A measured before/after comparison using the same `strace`-based method as the Evidence section, recorded in `CHANGELOG.md`.

### Excluded (explicit follow-up, not this plan)
- Batching the 12 repetition-body calls (`preformat_rep_bodies`) across a file — smaller win (35% vs 62% of per-pass calls), separate call site, separate plan.
- Batching the item/block invocation-formatting calls (`format_invocation_inner`) — smallest win (3%) in this fixture, separate plan.
- Any change to `run_rustfmt_no_macro` / `final_format_pass` (the 4 non-shadow calls) — out of scope.
- Any change to formatting *output* — this plan must not change what gets formatted or how, only how many processes it takes to do it.

## Architecture

### Current shape (per convergence pass, in `format_source_once`)

```rust
for (index, definition) in definitions.iter().enumerate().rev() {
    match format_definition_once(&text, definition, rustfmt_path, edition, config_path) {
        Ok(candidate) => match ensure_tokens_preserved(&text, &candidate) {
            Ok(()) => text = candidate,
            Err(error) => skipped_reasons[index] = Some(format!("lossless check failed: {error}")),
        },
        Err(error) => skipped_reasons[index] = Some(format!("shadow formatting failed: {error}")),
    }
}
```

`format_definition_once` builds one shadow file per definition (batching only that definition's own arms) and calls `run_rustfmt` once. With N definitions, that's N calls.

### New shape

Split definition processing into two phases:

**Phase 1 (unchanged):** definitions that do *not* `supports_deep_format` still go through `format_definition_without_brace_bodies` in the existing reverse loop — no `rustfmt` call, no behavior change.

**Phase 2 (new):** re-parse the text after phase 1 (byte offsets may have shifted), collect every definition that *does* `supports_deep_format`, and try one batched call:

```rust
fn format_definitions_batch(
    source: &str,
    definitions: &[&MacroDef],
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let marker_prefix = unique_prefix(source);
    let mut all_replaced_bodies_str: Vec<String> = Vec::new();
    let mut all_mappings: Vec<Mapping> = Vec::new();
    for definition in definitions {
        for arm in &definition.arms {
            // identical per-arm body extraction as format_definition_once:
            // normalize_body_indent, replace_macro_syntax_text, preformat_rep_bodies
            all_replaced_bodies_str.push(inner_str);
            all_mappings.push(mapping);
        }
    }
    let shadow_code = build_shadow_file_from_strings(&all_replaced_bodies_str, &marker_prefix);
    let formatted_shadow = run_rustfmt(&shadow_code, rustfmt_path, edition, config_path)?;
    let owned: Vec<MacroDef> = definitions.iter().map(|d| (*d).clone()).collect();
    Ok(apply_formatting(source, &owned, &formatted_shadow, &all_mappings))
}
```

This works with **zero changes** to `build_shadow_file_from_strings`, `split_shadow_into_arms`, or `apply_formatting` — all three already operate on flat, positionally-ordered lists spanning an arbitrary number of definitions; today they just happen to always be called with exactly one definition's arms. `apply_formatting` already loops `for definition in macro_defs`, advancing `section_position`/`mapping_position` by `definition.arms.len()` per definition — this is already multi-definition-ready.

Orchestration in `format_source_once`:

```rust
if !deep_definitions.is_empty() {
    match format_definitions_batch(&text, &deep_definitions, rustfmt_path, edition, config_path) {
        Ok(candidate) if ensure_tokens_preserved(&text, &candidate).is_ok() => {
            text = candidate;
        }
        _ => {
            // fallback: existing one-call-per-definition sequential loop, unchanged,
            // so a single bad definition among many healthy ones is still isolated
            // and reported SKIPPED individually instead of failing the whole batch.
            for definition in deep_definitions.iter().rev() {
                // ... exact existing format_definition_once + ensure_tokens_preserved logic ...
            }
        }
    }
}
```

The fallback makes this strictly additive from a safety standpoint: the worst case (something in the batch fails the lossless check) is exactly today's behavior and today's call count. The common case (everything formats cleanly, which is true for 100% of the current fixture/corpus) drops from N calls to 1.

### Call-count instrumentation

Add a process-wide `AtomicUsize` counter in `formatter.rs`, incremented once per successful `Command::spawn` in both `run_rustfmt` and `run_rustfmt_no_macro`, with `pub fn rustfmt_call_count() -> usize` and `pub fn reset_rustfmt_call_count()`. This replaces the throwaway `eprintln!` instrumentation used to gather the Evidence numbers with a permanent, test-usable primitive.

## Testing Strategy

1. **Perf regression test (new, drives the implementation):** format a fixture with several independent healthy `macro_rules!` definitions, assert `rustfmt_call_count()` stays at or below a small fixed ceiling regardless of definition count. Written first; fails against the current per-definition implementation; passes once batching lands.
2. **Existing full regression suite is the correctness gate:** all current `cargo test --release` (91 tests) and `python3 tests/run_fixtures.py` (100% exact-output conformance) must stay green with byte-identical output — this plan is not allowed to change what gets formatted.
3. **Fallback-path test:** extract the "is the batch safe to apply" branch into a small pure function, `accepted_batch_result(original: &str, batch_result: anyhow::Result<String>) -> Option<String>`, and unit-test it directly with synthetic `Ok`/`Err` values (including an `Ok` whose tokens don't match `original`). This proves the fallback decision is correct without needing a fake `rustfmt` binary, which would be fragile across the Linux/Windows/macOS targets this project bundles.
4. **Before/after measurement:** repeat the `strace`-based call count and `/usr/bin/time` wall-clock measurement from the Evidence section on the same fixture, record the actual improvement.

## Implementation Boundaries

- Does not touch `preformat_rep_bodies`, `format_macro_invocations`, or `run_rustfmt_no_macro` — those are explicitly out of scope (see Scope).
- Does not change `MAX_FORMAT_PASSES` (8) or the convergence/fixed-point logic in `format_source_with_report`.
- Does not change any public CLI flag, VS Code setting, or command.
- `format_definition_once` (the current per-definition function) is kept unchanged and reused as-is inside the fallback branch — no duplicated formatting logic.

## Completion Criteria

- `rustfmt_call_count()` for formatting `test-rs/src/examples/macro_heavy.rs` (21 definitions, already-formatted) drops from ~90 to a small, fixed-size number independent of definition count.
- `cargo test --release` in `rust-fmt-mf`: 91/91 passing, unchanged.
- `python3 tests/run_fixtures.py`: 100% exact-output conformance, unchanged, 0 new `SKIPPED` outcomes.
- Wall-clock time for the same fixture drops measurably (recorded, not promised as an exact number ahead of implementation).
