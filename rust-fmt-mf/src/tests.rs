use crate::mapper::*;
use crate::parser::*;
use crate::replacer::*;
use crate::shadow::*;
use crate::types::Mapping;
use std::sync::Mutex;

// Lock to serialize counter-sensitive tests, preventing concurrent mutations
// of the global RUSTFMT_CALL_COUNT by other tests running in parallel.
// Rule: any test in this file that reaches a formatting entry point, whether
// directly or transitively (calls `run_rustfmt`, `run_rustfmt_no_macro`,
// `format_source`/`format_source_once`/`format_source_with_report`, or any
// helper that calls into one of those), must acquire this lock first, or it
// will race with the counter-consuming tests under default-parallel
// `cargo test` and corrupt their measurements.
static COUNTER_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn marker_collision_is_idempotent() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = include_str!("../tests/fixtures/marker_collision.rs");
    let first = super::format_source_once(source, "rustfmt", "2021", None).unwrap();
    let second = super::format_source_once(&first.text, "rustfmt", "2021", None).unwrap();
    assert_eq!(first.text, second.text);
}

#[test]
fn real_macro_edge_cases_are_idempotent() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = include_str!("../tests/fixtures/real_macro_edge_cases.rs");
    let first = super::format_source_once(source, "rustfmt", "2021", None).unwrap();
    let second = super::format_source_once(&first.text, "rustfmt", "2021", None).unwrap();
    assert_eq!(first.text, second.text);
}

#[test]
fn real_main_fmt_is_idempotent() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = include_str!("../tests/fixtures/real_main_fmt.rs");
    let first = super::format_source_once(source, "rustfmt", "2021", None).unwrap();
    let second = super::format_source_once(&first.text, "rustfmt", "2021", None).unwrap();
    assert_eq!(first.text, second.text);
}

use proc_macro2::TokenStream;

#[test]
fn test_simple_macro() {
    let source = r#"
macro_rules! foo {
    ($x:expr) => { $x + 1 };
}
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "foo");
    assert_eq!(defs[0].arms.len(), 1);
    let arm = &defs[0].arms[0];
    let body = &source[arm.body_span.clone()];
    assert!(body.starts_with('{'));
    assert!(body.ends_with('}'));
    assert!(body.contains("$x + 1"));
}

#[test]
fn test_multi_arm_macro() {
    let source = r#"
macro_rules! multi {
    ($a:expr) => { $a + 1 };
    ($a:expr, $b:expr) => { $a + $b };
}
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "multi");
    assert_eq!(defs[0].arms.len(), 2);
    let body0 = &source[defs[0].arms[0].body_span.clone()];
    let body1 = &source[defs[0].arms[1].body_span.clone()];
    assert!(body0.contains("$a + 1"));
    assert!(body1.contains("$a + $b"));
}

#[test]
fn test_double_brace() {
    let source = r#"
macro_rules! rle {
    ($($x:expr),*) => {{
        let mut v = Vec::new();
        v.push(1);
    }};
}
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].arms.len(), 1);
    let body = &source[defs[0].arms[0].body_span.clone()];
    assert!(
        body.starts_with("{{"),
        "body should start with double brace"
    );
    assert!(body.ends_with("}}"), "body should end with double brace");
}

#[test]
fn test_field_accessor() {
    let source = r#"
macro_rules! field_accessor {
    ( $struct_name:ident, $( $field:ident : $ty:ty ),+ ) => {
        impl $struct_name {
        $(
            pub fn $field( &self) -> &$ty {
                &self.$field
            }
        )+
        }
    };
}
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].arms.len(), 1);
    let body = &source[defs[0].arms[0].body_span.clone()];
    assert!(body.contains("impl"));
    assert!(body.contains("pub fn $field"));
}

#[test]
fn test_nested_macro() {
    let source = r#"
macro_rules! outer {
    () => {
        macro_rules! inner {
            () => { 42 }
        }
    };
}
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1, "only outer macro should be found");
    assert_eq!(defs[0].name, "outer");
    assert_eq!(defs[0].arms.len(), 1);
}

#[test]
fn test_pattern_with_braces() {
    let source = r#"
macro_rules! pat {
    ({ $x:expr }) => { $x * 2 };
}
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].arms.len(), 1);
    let pattern = &source[defs[0].arms[0].pattern_span.clone()];
    assert!(pattern.contains('{'));
    assert!(pattern.contains('}'));
    let body = &source[defs[0].arms[0].body_span.clone()];
    assert!(body.contains("$x * 2"));
}

#[test]
fn test_raw_string_in_body() {
    let source = "\nmacro_rules! test {\n    ($x:expr) => {\n        let s = r#\"hello world\"#;\n        $x\n    };\n}\n";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "test");
    let body = &source[defs[0].arms[0].body_span.clone()];
    assert!(
        body.contains("hello world"),
        "body should contain raw string content"
    );
}

#[test]
fn test_multi_macro_with_raw_string() {
    let source = "\nmacro_rules! first {\n    ($x:expr) => {\n        let s = r#\"data\"#;\n        $x\n    };\n}\nmacro_rules! second {\n    () => { 42 };\n}\n";
    let defs = parse_macro_defs(source).unwrap();
    assert!(
        defs.len() >= 2,
        "should find at least 2 macros, found {}",
        defs.len()
    );
}

#[test]
fn test_no_macros() {
    let source = "fn main() { println!(\"hello\"); }";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 0);
}

#[test]
fn parser_ignores_macro_rules_inside_trivia_and_literals() {
    let source = r#"
const TEXT: &str = "macro_rules! fake { () => { 1 } }";
// macro_rules! comment { () => { 2 } }
macro_rules! real { () => { 3 }; }
"#;
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(
        defs.iter().map(|def| def.name.as_str()).collect::<Vec<_>>(),
        ["real"]
    );
}

#[test]
fn parser_preserves_unicode_and_literal_delimiters() {
    let source = "fn привет() {}\nmacro_rules! m { () => { let c = '}'; // }\n c }; }\n";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(
        &source[defs[0].span.clone()],
        "macro_rules! m { () => { let c = '}'; // }\n c }; }"
    );
}

#[test]
fn parser_supports_all_definition_and_transcriber_delimiters() {
    let source = "macro_rules! a (($x:expr) => [$x];);\nmacro_rules! b [($x:expr) => ($x);];";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].arms.len(), 1);
    assert_eq!(defs[1].arms.len(), 1);
}

#[test]
fn test_macro_heavy_file() {
    let source = include_str!("../../test-rs/src/examples/macro_heavy.rs");
    let defs = parse_macro_defs(source).unwrap();
    assert!(
        defs.len() >= 20,
        "should find at least 20 macros, found {}",
        defs.len()
    );
    for def in &defs {
        assert!(
            !def.arms.is_empty(),
            "macro {} should have at least 1 arm",
            def.name
        );
        for arm in &def.arms {
            let body = &source[arm.body_span.clone()];
            assert!(
                body.starts_with('{'),
                "body of {} should start with '{{', got: {:?}",
                def.name,
                &body[..body.len().min(20)]
            );
        }
    }
}

fn make_mapping(vars: &[(&str, &str)]) -> Mapping {
    let mut m = Mapping::new();
    for (placeholder, original) in vars {
        m.vars.insert(placeholder.to_string(), original.to_string());
    }
    m
}

#[test]
fn test_detect_arm_opener() {
    assert_eq!(
        detect_arm_opener("macro_rules! __rustfmt_mf_arm_0 {"),
        Some(0)
    );
    assert_eq!(
        detect_arm_opener("    macro_rules! __rustfmt_mf_arm_42 {"),
        Some(42)
    );
    assert_eq!(detect_arm_opener("fn foo() {"), None);
}

#[test]
fn test_restore_placeholders() {
    let mapping = make_mapping(&[("__m_0", "$x"), ("__m_1", "$y")]);
    let result = restore_placeholders("let x = __m_0 + __m_1;", &mapping);
    assert_eq!(result, "let x = $x + $y;");
}

#[test]
fn test_restore_longest_first() {
    let mapping = make_mapping(&[("__m_1", "$a"), ("__m_10", "$bb")]);
    let result = restore_placeholders("__m_10 + __m_1", &mapping);
    assert_eq!(result, "$bb + $a");
}

#[test]
fn test_map_arm_section_with_repetition() {
    let section = "    impl __m_0 {\n        __mf_rep_plus! {\n            __m_1\n        }\n    }";
    let mapping = make_mapping(&[("__m_0", "$struct_name"), ("__m_1", "$field")]);
    let result = map_arm_section(section, &mapping);
    assert!(result.contains("$("));
    assert!(result.contains(")+"));
    assert!(result.contains("$struct_name"));
    assert!(result.contains("$field"));
}

#[test]
fn test_map_arm_section_preserves_tuple_trailing_comma() {
    let mapping = Mapping::new();
    let result = map_arm_section("(\n    value,\n)", &mapping);
    assert_eq!(result, "(value,)");
}

#[test]
fn test_split_shadow_into_arms() {
    let shadow = "#![allow(unused_attributes, dead_code)]\n\nmacro_rules! __rustfmt_mf_arm_0 {\n    () => {\n        let x = 1;\n    };\n}\n\nmacro_rules! __rustfmt_mf_arm_1 {\n    () => {\n        let y = 2;\n    };\n}\n";
    let sections = split_shadow_into_arms(shadow);
    assert_eq!(sections.len(), 2);
    assert!(sections[0].contains("let x = 1"));
    assert!(sections[1].contains("let y = 2"));
}

#[test]
fn test_split_shadow_preserves_an_inner_block() {
    let shadow = "macro_rules! __rustfmt_mf_arm_0 {\n    () => {{\n        value\n    }};\n}\n";
    let sections = split_shadow_into_arms(shadow);
    assert_eq!(sections, ["{\n        value\n    }"]);
}

fn replace_and_map(source: &str) -> (String, Mapping) {
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let replaced = replace_macro_syntax(&tokens, &mut mapping);
    (replaced.to_string(), mapping)
}

#[test]
fn test_simple_var() {
    let (result, mapping) = replace_and_map("$x");
    assert!(result.starts_with("__m_"));
    assert_eq!(mapping.vars.len(), 1);
    let placeholder = &result;
    let original = mapping.restore(placeholder).unwrap();
    assert_eq!(original, "$x");
}

#[test]
fn test_var_with_type() {
    let (result, mapping) = replace_and_map("$x : expr");
    assert!(result.starts_with("__m_"));
    assert_eq!(mapping.vars.len(), 1);
    let placeholder = result.trim();
    let original = mapping.restore(placeholder).unwrap();
    assert_eq!(original, "$x:expr");
}

#[test]
fn test_repetition_star() {
    let source = "$ ( $ x : expr ) *";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("__mf_rep_star"));
    assert!(!result.contains("__mf_rep_plus"));
}

#[test]
fn test_repetition_plus() {
    let source = "$ ( $ x : ident ) +";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("__mf_rep_plus"));
    assert!(!result.contains("__mf_rep_star"));
}

#[test]
fn test_repetition_question_tokenstream() {
    let source = "$ ( $ x : expr ) ?";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("__mf_rep_question"));
    assert!(!result.contains("__mf_rep_star"));
    assert!(!result.contains("__mf_rep_plus"));
}

#[test]
fn test_repetition_with_separator() {
    let source = "$ ( $ field : ident ) , +";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("__mf_rep_plus_comma"));
}

#[test]
fn test_nested_repetition() {
    let source = "$ ( $ ( $ x : expr ) * ) +";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("__mf_rep_star"));
    assert!(result.contains("__mf_rep_plus"));
}

#[test]
fn test_field_accessor_body() {
    let source = "impl $struct_name { $( pub fn $field( &self) -> &$ty { &self.$field } )+ }";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("impl __m_"));
    assert!(result.contains("__mf_rep_plus"));
    assert!(!result.contains("$struct_name"));
    assert!(!result.contains("$field"));
}

#[test]
fn test_crate_replacement() {
    let (result, mapping) = replace_and_map("$crate");
    assert_eq!(mapping.vars.len(), 1);
    let placeholder = result.trim();
    let original = mapping.restore(placeholder).unwrap();
    assert_eq!(original, "$crate");
}

#[test]
fn test_crate_path() {
    let source = "$crate :: module :: Type";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(result.contains("__m_"));
    assert!(result.contains("module"));
    assert!(result.contains("Type"));
}

#[test]
fn test_multiple_vars_unique_ids() {
    let source = "$x + $y + $z";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    let placeholders: Vec<&str> = result
        .split_whitespace()
        .filter(|w| w.starts_with("__m_"))
        .collect();
    assert_eq!(placeholders.len(), 3);
}

#[test]
fn test_preserves_non_macro_tokens() {
    let source = "let x = 1 + 2;";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    let r: String = result.chars().filter(|c| !c.is_whitespace()).collect();
    let s: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(r, s);
    assert!(mapping.vars.is_empty());
}

#[test]
fn test_dollar_not_followed_by_valid() {
    let source = "$";
    let tokens: TokenStream = source.parse().unwrap();
    let mut mapping = Mapping::new();
    let _result = replace_macro_syntax(&tokens, &mut mapping).to_string();
    assert!(mapping.vars.is_empty());
}

#[test]
fn test_text_repetition_question() {
    let source = "$(,)?";
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax_text(source, &mut mapping);
    assert!(result.contains("__mf_rep_question"));
    assert!(!result.contains("$(,"));
}

#[test]
fn test_text_repetition_question_with_var() {
    let source = "$($x:expr)?";
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax_text(source, &mut mapping);
    assert!(result.contains("__mf_rep_question"));
    assert!(result.contains("__m_"));
}

#[test]
fn test_text_repetition_question_nested() {
    let source = "$(($($field:ty),*))?";
    let mut mapping = Mapping::new();
    let result = replace_macro_syntax_text(source, &mut mapping);
    assert!(result.contains("__mf_rep_question"));
    assert!(result.contains("__mf_rep_star_comma"));
    assert!(!result.contains("$("));
}

fn tokens(s: &str) -> TokenStream {
    s.parse().unwrap()
}

#[test]
fn test_single_arm() {
    let body = tokens("let x = 1;");
    let (shadow, count) = build_shadow_file(&[body]);
    assert_eq!(count, 1);
    assert!(
        shadow.contains("macro_rules ! __rustfmt_mf_arm_0")
            || shadow.contains("macro_rules! __rustfmt_mf_arm_0")
    );
    assert!(shadow.contains("let x = 1"));
}

#[test]
fn test_multiple_arms() {
    let bodies = vec![
        tokens("let x = 1;"),
        tokens("let y = 2;"),
        tokens("let z = 3;"),
    ];
    let (shadow, count) = build_shadow_file(&bodies);
    assert_eq!(count, 3);
    assert!(
        shadow.contains("macro_rules ! __rustfmt_mf_arm_0")
            || shadow.contains("macro_rules! __rustfmt_mf_arm_0")
    );
    assert!(
        shadow.contains("macro_rules ! __rustfmt_mf_arm_1")
            || shadow.contains("macro_rules! __rustfmt_mf_arm_1")
    );
    assert!(
        shadow.contains("macro_rules ! __rustfmt_mf_arm_2")
            || shadow.contains("macro_rules! __rustfmt_mf_arm_2")
    );
    assert!(shadow.contains("let x = 1"));
    assert!(shadow.contains("let y = 2"));
    assert!(shadow.contains("let z = 3"));
}

#[test]
fn test_with_replaced_body() {
    let body = tokens("__mf_rep_plus ! { __m_1 }");
    let (shadow, count) = build_shadow_file(&[body]);
    assert_eq!(count, 1);
    assert!(
        shadow.contains("macro_rules ! __rustfmt_mf_arm_0")
            || shadow.contains("macro_rules! __rustfmt_mf_arm_0")
    );
    assert!(shadow.contains("__mf_rep_plus"));
    assert!(shadow.contains("__m_1"));
}

#[test]
fn test_empty_arms() {
    let (shadow, count) = build_shadow_file(&[]);
    assert_eq!(count, 0);
    assert!(shadow.contains("allow"));
    assert!(shadow.contains("unused_attributes"));
}

#[test]
fn test_shadow_has_allow_attributes() {
    let body = tokens("let x = 1;");
    let (shadow, _) = build_shadow_file(&[body]);
    assert!(shadow.contains("allow"));
    assert!(shadow.contains("dead_code"));
    assert!(
        shadow.contains("macro_rules ! __rustfmt_mf_arm_0")
            || shadow.contains("macro_rules! __rustfmt_mf_arm_0")
    );
}

#[test]
fn rustfmt_call_count_tracks_successful_spawns() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::formatter::reset_rustfmt_call_count();
    assert_eq!(crate::formatter::rustfmt_call_count(), 0);
    crate::formatter::run_rustfmt("fn main() {}", "rustfmt", "2021", None).unwrap();
    assert_eq!(crate::formatter::rustfmt_call_count(), 1);
    crate::formatter::run_rustfmt_no_macro("fn main() {}", "rustfmt", "2021", None).unwrap();
    assert_eq!(crate::formatter::rustfmt_call_count(), 2);
}

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

#[test]
fn formatting_many_independent_macros_uses_few_rustfmt_calls() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    // Before batching, this needed one rustfmt call per definition per
    // convergence pass (measured: 7 calls, unbatched). Batched, it needs
    // roughly one shadow call plus one final-pass call per convergence pass
    // (measured: 3 calls). The ceiling below is the measured value (3) plus
    // 2 calls of headroom, so one extra convergence pass forced by a future
    // rustfmt version bump doesn't immediately red this test in CI. This
    // must NOT scale with the number of definitions.
    assert!(
        calls <= 5,
        "expected batched formatting of 5 independent macros to use at most 5 rustfmt calls, used {calls}"
    );
}

#[test]
fn overlapping_definition_spans_do_not_panic_the_batch() {
    // parse_macro_defs's attribute/doc-comment heuristic can pull a later
    // definition's span.start backward into an earlier definition's
    // trailing-comment line when that comment ends with `]` (it looks like
    // an attribute such as `#[foo]` from the end). This produces a partial
    // span overlap that survives the containment filter in parser.rs.
    // Before the fix, feeding such an overlapping pair into
    // format_definitions_batch's single apply_formatting call panicked
    // (`byte range starts at X but ends at Y`) because apply_formatting
    // assumes strictly ascending, non-overlapping spans. This must instead
    // format cleanly, falling back to the one-call-per-definition path for
    // just the overlapping definitions.
    //
    // This calls into code that increments the shared rustfmt-call counter,
    // so it must hold COUNTER_TEST_LOCK like every other test that does,
    // or it can race against formatting_many_independent_macros_uses_few_rustfmt_calls
    // under default-parallel `cargo test` and corrupt its measurement.
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "macro_rules! a {\n    () => { 1 };\n} // see [docs]\n\nmacro_rules! b {\n    () => { 2 };\n}\n";
    let formatted = super::format_source(source, "rustfmt", "2021", None)
        .expect("overlapping definitions must format without erroring or panicking");
    assert!(formatted.contains("macro_rules! a"));
    assert!(formatted.contains("macro_rules! b"));
    assert!(formatted.contains("// see [docs]"));
    // Must be idempotent too, not just non-panicking on the first pass.
    let second = super::format_source(&formatted, "rustfmt", "2021", None).unwrap();
    assert_eq!(formatted, second);
}

// The following two tests exercise `apply_deep_definitions_batch` directly
// (the actual fallback LOOP used inside `format_source_once`, not just the
// pure `accepted_batch_result` decision function tested above). They inject
// a fake `batch` closure that deliberately fails, so that a real `rustfmt`
// binary is only needed for the fallback's per-definition formatting calls
// (via the real `format_definition_once`), never for the batch attempt
// itself. This proves the branch this whole plan exists to guarantee: when
// the combined batch call is rejected, the fallback still formats every
// healthy definition correctly instead of leaving the file untouched or
// letting corrupted output through.

#[test]
fn apply_deep_definitions_batch_falls_back_when_batch_call_errors() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "macro_rules! one {\n    ($x:expr) => {$x+1};\n}\n\nmacro_rules! two {\n    ($x:expr) => {$x+2};\n}\n\nmacro_rules! three {\n    ($x:expr) => {$x+3};\n}\n";
    let definitions = parse_macro_defs(source).unwrap();
    assert_eq!(definitions.len(), 3);
    let batchable: Vec<(usize, &crate::types::MacroDef)> = definitions.iter().enumerate().collect();
    let mut skipped_reasons = vec![None; definitions.len()];

    let result = super::apply_deep_definitions_batch(
        source.to_string(),
        &batchable,
        &mut skipped_reasons,
        |_text, _defs| Err(anyhow::anyhow!("simulated batch rustfmt failure")),
        "rustfmt",
        "2021",
        None,
    );

    // The fallback loop must have run format_definition_once per
    // definition, individually formatting each healthy macro body, instead
    // of leaving the whole file untouched (or, worse, silently corrupted).
    assert!(
        result.contains("$x + 1"),
        "definition `one` should be individually formatted by the fallback: {result}"
    );
    assert!(
        result.contains("$x + 2"),
        "definition `two` should be individually formatted by the fallback: {result}"
    );
    assert!(
        result.contains("$x + 3"),
        "definition `three` should be individually formatted by the fallback: {result}"
    );
    assert_eq!(
        skipped_reasons,
        vec![None, None, None],
        "all three healthy definitions should format cleanly via the fallback loop, none should be SKIPPED"
    );
}

#[test]
fn apply_deep_definitions_batch_falls_back_when_batch_result_corrupts_tokens() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "macro_rules! one {\n    ($x:expr) => {$x+1};\n}\n\nmacro_rules! two {\n    ($x:expr) => {$x+2};\n}\n";
    let definitions = parse_macro_defs(source).unwrap();
    assert_eq!(definitions.len(), 2);
    let batchable: Vec<(usize, &crate::types::MacroDef)> = definitions.iter().enumerate().collect();
    let mut skipped_reasons = vec![None; definitions.len()];

    // Simulate a batch call that "succeeds" but silently changes a
    // significant token (the `1` in `one`'s body becomes `999`).
    // `accepted_batch_result`'s `ensure_tokens_preserved` check must reject
    // this candidate, driving `apply_deep_definitions_batch` into the same
    // fallback loop as the error case above, rather than applying
    // token-corrupted output to the file.
    let result = super::apply_deep_definitions_batch(
        source.to_string(),
        &batchable,
        &mut skipped_reasons,
        |text, _defs| Ok(text.replacen("+1", "+999", 1)),
        "rustfmt",
        "2021",
        None,
    );

    assert!(
        !result.contains("999"),
        "token-corrupting batch output must never be applied to the file: {result}"
    );
    assert!(
        result.contains("$x + 1"),
        "definition `one` should still be correctly formatted via the fallback: {result}"
    );
    assert!(
        result.contains("$x + 2"),
        "definition `two` (never touched by the fake batch closure) should still be correctly formatted via the fallback: {result}"
    );
    assert_eq!(
        skipped_reasons,
        vec![None, None],
        "both definitions should format cleanly via the fallback loop, none should be SKIPPED"
    );
}

#[test]
fn crlf_input_with_a_comment_formats_without_erroring() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "//! doc comment\r\nmacro_rules! one {\r\n    ($x:expr) => { $x + 1 };\r\n}\r\n\r\npub fn use_one() {\r\n    one!(1);\r\n}\r\n";
    let result = super::format_source(source, "rustfmt", "2021", None);
    assert!(
        result.is_ok(),
        "CRLF input with a leading comment must not error: {:?}",
        result.err()
    );
}

#[test]
fn crlf_input_round_trips_and_preserves_line_ending_style() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "//! doc comment\r\nmacro_rules! one {\r\n    ($x:expr) => { $x + 1 };\r\n}\r\n\r\npub fn use_one() {\r\n    one!(1);\r\n}\r\n";
    let result = super::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(
        result.contains("\r\n"),
        "output must use CRLF to match the original file's line-ending style: {result:?}"
    );
    // Every line ending must be CRLF, not a bare LF: once every "\r\n" pair
    // is removed, no bare '\n' (or stray '\r') should remain.
    let stripped = result.replace("\r\n", "");
    assert!(
        !stripped.contains('\n') && !stripped.contains('\r'),
        "found a line ending that isn't a paired CRLF: {result:?}"
    );
}

#[test]
fn crlf_report_spans_point_at_the_original_crlf_source() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "//! doc comment\r\nmacro_rules! one {\r\n    ($x:expr) => { $x + 1 };\r\n}\r\n";
    let report = super::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    let outcome = &report.macros[0];
    // The parser's span for a definition already includes an immediately
    // preceding doc comment (pre-existing, CRLF-independent behavior,
    // verified identically on plain LF input) — the CRLF-awareness this
    // test targets is that the slice below is byte-exact against the
    // *original* CRLF source, not shifted by the internal LF normalization.
    assert_eq!(
        &source[outcome.span.clone()],
        "//! doc comment\r\nmacro_rules! one {\r\n    ($x:expr) => { $x + 1 };\r\n}",
        "the reported span must slice the ORIGINAL CRLF source correctly, not a shifted position"
    );
}

#[test]
fn lf_only_input_is_completely_unaffected_by_crlf_handling() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "//! doc comment\nmacro_rules! one {\n    ($x:expr) => { $x + 1 };\n}\n\npub fn use_one() {\n    one!(1);\n}\n";
    let result = super::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(
        !result.contains('\r'),
        "LF-only input must never gain a CR: {result:?}"
    );
}

#[test]
fn identical_rustfmt_requests_are_served_from_the_memo() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::formatter::reset_rustfmt_call_count();
    let source = "fn memo_probe() {let x=1;}";
    crate::formatter::run_rustfmt_no_macro(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(crate::formatter::rustfmt_call_count(), 1);
    // Same binary, args and stdin: rustfmt is deterministic, so the second
    // and third asks must not spawn anything.
    let first = crate::formatter::run_rustfmt_no_macro(source, "rustfmt", "2021", None).unwrap();
    let second = crate::formatter::run_rustfmt_no_macro(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(first, second);
    assert_eq!(crate::formatter::rustfmt_call_count(), 1);
    // A different edition is a different question and must still spawn.
    crate::formatter::run_rustfmt_no_macro(source, "rustfmt", "2018", None).unwrap();
    assert_eq!(crate::formatter::rustfmt_call_count(), 2);
}

#[test]
fn rejected_input_is_memoized_too() {
    let _guard = COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::formatter::reset_rustfmt_call_count();
    let broken = "fn ( { this is not rust";
    assert!(crate::formatter::run_rustfmt_no_macro(broken, "rustfmt", "2021", None).is_err());
    assert_eq!(crate::formatter::rustfmt_call_count(), 1);
    assert!(crate::formatter::run_rustfmt_no_macro(broken, "rustfmt", "2021", None).is_err());
    assert_eq!(crate::formatter::rustfmt_call_count(), 1);
}

#[test]
fn macro_rules_inside_another_macros_invocation_is_not_a_definition() {
    // `quote! { macro_rules! .. }` is argument text belonging to `quote`,
    // not an item of this file. Parsing it as a definition rewrote the
    // insides of every `quote!` block in the crate's own source.
    let source = r#"
fn build() -> TokenStream {
    quote::quote! {
        macro_rules! __mf_rep_star { ($($t:tt)*) => { $($t)* }; }
    }
}
"#;
    let definitions = crate::parser::parse_macro_defs(source).unwrap();
    assert!(
        definitions.is_empty(),
        "expected no definitions, got {:?}",
        definitions.iter().map(|d| &d.name).collect::<Vec<_>>()
    );
    // A real top-level definition next to one still parses.
    let with_real = format!(
        "macro_rules! real {{ () => {{}}; }}
{source}"
    );
    let definitions = crate::parser::parse_macro_defs(&with_real).unwrap();
    assert_eq!(
        definitions
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        vec!["real"]
    );
}
