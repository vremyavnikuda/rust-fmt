# Safe Macro Formatting and Coverage Design

## Goal

Make macro formatting fail-safe, measurable, and reproducible on Linux and Windows. For every valid Rust input, the extension must preserve the program's significant token stream, return syntactically valid Rust, and produce byte-identical output on a second formatting call.

Deep formatting is applied only when an arm can be represented safely as Rust-like syntax. An opaque or unsupported macro DSL is preserved unchanged and reported as `SKIPPED`; it is never rewritten heuristically.

The project reports three separate percentages instead of one ambiguous number:

- **Safety coverage:** cases that exit successfully, preserve significant tokens, remain valid Rust, and are idempotent.
- **Golden coverage:** supported fixtures whose output exactly matches the reviewed `.expected` file.
- **Deep-format coverage:** macro definitions formatted by the native engine rather than deliberately preserved as opaque DSL.

Safety coverage must always be 100%. Golden and deep-format coverage may increase as explicit support is added, but `SKIPPED` must never be counted as a pass for deep formatting.

## Confirmed Failure Modes

The current implementation has five independent classes of failure:

1. `parser.rs` scans bytes without a complete Rust lexical model. It can treat `macro_rules!` inside a string as code, terminate a body on `}` inside a character literal or comment, miss non-brace macro definitions, and mishandle Unicode.
2. Matcher newlines are collapsed without respecting line comments. A `//` comment can consume the rest of a matcher and corrupt the macro span.
3. Global textual replacements run across literals, comments, and user identifiers. They can change string values and collide with names such as `__m_0` or marker comments such as `__mf_nm_0__`.
4. The TypeScript layer rewrites the entire document with `normalizeMacroSpacing` before invoking the native formatter and applies a second heuristic macro formatter on fallback.
5. The fixture runner uses a manual list. Six checked-in fixtures are currently omitted, so a green run does not mean the corpus was fully exercised. The bundled Linux binary can also differ from the current source build without a failing check.

## Scope

### Included

- Replace byte-level macro discovery and delimiter matching with a Rust-aware lexer.
- Preserve exact source ranges for strings, raw strings, byte/C strings, character literals, Unicode identifiers, comments, and all three delimiter kinds.
- Remove semantic text rewrites from the TypeScript path.
- Make every native transformation subject to token preservation, syntax, and idempotence checks.
- Make fixture discovery automatic.
- Add adversarial regression cases for all confirmed failure modes.
- Audit both golden fixtures and the complete `test-rs` corpus.
- Verify that the bundled current-platform binary is byte-identical to the release artifact produced by the F5 build.
- Keep the extension and native binary working on Linux x86_64 and Windows x86_64.

### Excluded

- A formatter for every possible third-party macro DSL.
- Runtime downloads or compiling Rust during extension installation.
- Expanding macros or trying to prove equivalence by comparing compiler expansion output.
- Changing the public VS Code settings names.

## Architecture

### 1. Lexical source model

Add `ra-ap-rustc_lexer` to `rust-fmt-mf`. A single lexer pass produces tokens with exact byte ranges. Whitespace and comments remain available as trivia; literals and identifiers are never reconstructed byte by byte.

`parser.rs` consumes this token list to locate the sequence `macro_rules`, `!`, macro name, and the definition delimiter. It matches `()`, `[]`, and `{}` with a typed delimiter stack. It then identifies each matcher, `=>`, and transcriber using token boundaries rather than characters. The transcriber may use any delimiter accepted by Rust.

Every returned `MacroDef` and `MacroArm` range must start and end on UTF-8 character boundaries. Parser errors identify the macro name and byte range. A construct that is valid Rust but not supported for deep formatting becomes `SKIPPED`, not a partial parse.

### 2. Lossless transformations

Formatting operates on source ranges and only changes whitespace between known tokens. Literal contents, comments, identifiers, punctuation, delimiters, and repetition operators are copied from the original source.

The current global replacements in `normalize_body_spacing`, `normalize_inner_spacing`, `normalizeMacroSpacing`, and related helpers are removed from the active path. Any remaining whitespace normalization works over lexer gaps, never over raw strings.

Shadow Rust remains the way to reuse `rustfmt` for Rust-like arm bodies. Synthetic identifiers and final-pass markers use a nonce that is first checked to be absent from the source. Restoration is performed only at recorded marker occurrences. A missing, duplicate, or moved marker is an invariant violation and fails closed.

An arm that cannot be converted into valid shadow Rust is restored byte-for-byte and marked `SKIPPED`. Other arms in the same file may still be formatted.

### 3. Safety oracle

The library exposes a formatting result containing formatted text and per-macro outcomes:

```text
FormatResult
  text
  macros[]: FORMATTED | UNCHANGED | SKIPPED(reason)
```

Before the CLI returns formatted text, it enforces:

1. **Token preservation:** tokenize input and output; after excluding whitespace, compare token kind and exact lexeme in order. Comments are included, so moving a comment across code or swallowing code into a line comment fails.
2. **Syntax validation:** ordinary stable `rustfmt` must accept the complete result.
3. **Idempotence:** one internal formatting pass over the result must produce identical bytes. The public API does not hide drift with an arbitrary four-pass loop.

If token preservation or idempotence fails, the native process exits non-zero and emits no replacement document. The extension then runs ordinary `rustfmt` on the original input and logs the native failure. A safe per-arm `SKIPPED` is not a process failure.

Diagnostics use stderr and contain the macro name plus status and reason. Stdout remains formatted Rust so the existing extension process interface stays simple.

### 4. VS Code integration

`RustFormatter.formatWithRustfmt` sends the original document directly to the native binary. `normalizeMacroSpacing` and `normalizeMacroBodies` are removed from the formatting path.

When the native process succeeds, stderr diagnostics are copied to the extension output channel. When it fails, ordinary `rustfmt` receives the original text, not a heuristically normalized intermediate value. This preserves the existing user-visible fallback while preventing TypeScript from corrupting macro content.

The extension logs the selected binary path and SHA-256 at activation or first use. This makes a stale F5 binary immediately visible.

### 5. Acceptance runner

`rust-fmt-mf/tests/run_fixtures.py` discovers every `tests/fixtures/*.rs` file with a sibling `.expected` file. No manual fixture list remains.

For each golden fixture it checks:

1. native exit status;
2. exact golden output;
3. token preservation;
4. stable `rustfmt` syntax validation;
5. byte-identical second pass;
6. absence of unexpected `SKIPPED` results.

The runner also recursively audits every Rust file under `test-rs/src`. Corpus cases require token preservation, syntax, idempotence, and an explicit per-macro outcome. The summary prints exact counts and percentages for safety, golden output, and deep formatting.

Failures are classified as:

- `FORMAT_ERROR`
- `GOLDEN_DIFF`
- `TOKEN_CHANGED`
- `SYNTAX_ERROR`
- `NON_IDEMPOTENT`
- `SKIPPED`

Diagnostics remain under `rust-fmt-mf/target/macro-audit/` and include input, actual output, expected output when applicable, stderr, second-pass output, token diff, and text diff.

### 6. Artifact verification

The F5 prelaunch task continues to build the release binary for the current platform. After copying, the build script verifies that the source artifact and `bin/<platform>-<arch>/rust-fmt-mf[.exe]` have the same SHA-256.

The local acceptance command compares the bundled binary with the just-built release artifact before running the corpus. CI performs the same sequence independently on Linux x86_64 and Windows x86_64. No Windows artifact is rewritten during a Linux build and vice versa.

## Regression Corpus

Add focused fixtures for:

- matcher line comments;
- braces and parentheses inside line/block comments;
- character and byte literals containing delimiters;
- ordinary, raw, byte, raw-byte, C, and raw-C strings;
- Unicode identifiers and comments;
- the text `macro_rules!` inside literals and comments;
- user identifiers that resemble every synthetic marker;
- definition and transcriber delimiters `()`, `[]`, and `{}`;
- arbitrary legal repetition separators;
- nested repetitions and macro-generating macros;
- short tuples whose trailing comma is semantically significant;
- opaque DSL arms that must be preserved and reported as `SKIPPED`.

Each regression begins as a failing test. Cases whose generated macro is invocable include an invocation in a compilable corpus crate so `cargo check --all-targets` exercises the expansion.

## Implementation Boundaries

- `rust-fmt-mf/src/parser.rs`: lexical tokens, delimiter stack, macro/arm spans.
- `rust-fmt-mf/src/types.rs`: per-macro outcomes and formatting report.
- `rust-fmt-mf/src/lib.rs`: orchestration and safety gates.
- `rust-fmt-mf/src/replacer.rs`, `mapper.rs`, `shadow.rs`: lossless marker and whitespace handling.
- `rust-fmt-mf/tests/`: regression fixtures and complete acceptance runner.
- `src/formatter.ts`: original-text native call, safe fallback, diagnostics, binary hash logging.
- `scripts/build_current.py`: artifact equality check.
- CI workflow: native Linux and Windows acceptance runs.

No unrelated UI, settings, command, or workspace-formatting changes are included.

## Completion Criteria

The work is complete when:

- all checked-in fixtures are automatically discovered;
- safety coverage is 100% for fixtures and `test-rs`;
- golden coverage is 100% for the supported fixture set;
- every macro has an explicit `FORMATTED`, `UNCHANGED`, or `SKIPPED` outcome;
- the confirmed corruption examples preserve their original significant tokens;
- the `commented_matcher` case formats without error;
- formatting is byte-idempotent after one public call;
- `cargo check --all-targets` succeeds on the formatted corpus;
- F5 rebuilds and verifies the current Linux or Windows bundled binary;
- Linux and Windows native CI checks pass;
- no commits are created by the implementation workflow.
