use rust_fmt_mf::formatter::run_rustfmt;
use rust_fmt_mf::parser::parse_macro_defs;
use rust_fmt_mf::replacer::replace_macro_syntax;
use rust_fmt_mf::shadow::build_shadow_file;
use rust_fmt_mf::types::Mapping;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_simple_pipeline() {
    let source = "macro_rules! bad_macro {\n    ($x:expr) => {\n            let val = $x + 1 * 2 / 3;\n        println!(\"value: {}\", val)\n    };\n}\n";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].arms.len(), 1);
    let body_text = &source[defs[0].arms[0].body_span.clone()];
    let body_tokens: proc_macro2::TokenStream = body_text.parse().unwrap();
    let mut mapping = Mapping::new();
    let replaced = replace_macro_syntax(&body_tokens, &mut mapping);
    let inner = strip_outer_group(replaced);
    let (shadow, count) = build_shadow_file(&[inner]);
    assert_eq!(count, 1);
    let formatted = run_rustfmt(&shadow, "rustfmt", "2021", None).unwrap();
    let _result = rust_fmt_mf::mapper::apply_formatting(source, &defs, &formatted, &[mapping]);
}

#[test]
fn test_define_enum_invocation() {
    let source = "define_enum!(    MyGeneratedEnum  {
        Alpha,
        Beta(i32),
        Gamma(String, i32),
    }
);\n";
    let result = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(
        result,
        "define_enum!(MyGeneratedEnum {\n    Alpha,\n    Beta(i32),\n    Gamma(String, i32),\n});\n"
    );
}

fn strip_outer_group(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::{Delimiter, TokenTree};
    let mut iter = tokens.into_iter();
    match iter.next() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace && iter.next().is_none() => {
            g.stream()
        }
        _ => proc_macro2::TokenStream::new(),
    }
}

#[test]
fn test_multi_arm_pipeline() {
    let source = "macro_rules! multi {\n    ($a:expr) => { $a + 1 };\n    ($a:expr, $b:expr) => { $a + $b };\n    () => { 42 };\n}\n";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs[0].arms.len(), 3);
    let mut all_bodies = Vec::new();
    let mut all_mappings = Vec::new();
    for arm in &defs[0].arms {
        let body_text = &source[arm.body_span.clone()];
        let body_tokens: proc_macro2::TokenStream = body_text.parse().unwrap();
        let mut mapping = Mapping::new();
        let replaced = replace_macro_syntax(&body_tokens, &mut mapping);
        let inner = strip_outer_group(replaced);
        all_bodies.push(inner);
        all_mappings.push(mapping);
    }
    let (shadow, _) = build_shadow_file(&all_bodies);
    let formatted = run_rustfmt(&shadow, "rustfmt", "2021", None).unwrap();
    let _result = rust_fmt_mf::mapper::apply_formatting(source, &defs, &formatted, &all_mappings);
}

#[test]
fn test_struct_with_bounds_pipeline() {
    let source = "macro_rules! struct_with_bounds {\n    (#[$meta:meta] $vis:vis struct $name:ident<$($param:ident),+> where $($bound:ident : $trait:path),+ $(,)?{$($field:ident : $ty:ty),+ $(,)?}) => {\n                        #[$meta]\n        $vis struct $name<$($param),+>\n                    where\n            $($param: $trait),+\n        {\n                                        $(pub $field: $ty),+\n                 }\n    };\n}\n";
    let result = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    // The body must be formatted — all lines at consistent indent
    assert!(
        result.contains("        #[$meta]"),
        "Expected formatted attr line"
    );
    assert!(
        result.contains("        $vis struct $name<$($param),+>"),
        "Expected formatted struct line"
    );
    assert!(
        result.contains("        where"),
        "Expected formatted where line"
    );
    assert!(
        result.contains("        where\n            $($param: $trait),+"),
        "Expected where clause indent"
    );
    assert!(
        result.contains("        {\n            $(pub $field: $ty),+"),
        "Expected body indent"
    );
}

#[test]
fn test_impl_for_converges_in_one_call() {
    let source = include_str!("fixtures/impl_for.rs");
    let once = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    let twice = rust_fmt_mf::format_source(&once, "rustfmt", "2021", None).unwrap();
    assert_eq!(once, twice);
}

fn assert_tokens_preserved(input: &str, output: &str) {
    let before = rust_fmt_mf::parser::significant_tokens(input).unwrap();
    let after = rust_fmt_mf::parser::significant_tokens(output).unwrap();
    assert_eq!(
        before
            .iter()
            .map(|token| (&token.kind, &token.text))
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(|token| (&token.kind, &token.text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn real_macro_heavy_matches_user_golden() {
    let input = include_str!("fixtures/real_macro_heavy.rs");
    let expected = include_str!("fixtures/real_macro_heavy.expected");
    let actual = rust_fmt_mf::format_source(input, "rustfmt", "2021", None).unwrap();

    assert_tokens_preserved(input, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn real_macro_edge_cases_match_golden() {
    let input = include_str!("fixtures/real_macro_edge_cases.rs");
    let expected = include_str!("fixtures/real_macro_edge_cases.expected");
    let actual = rust_fmt_mf::format_source(input, "rustfmt", "2021", None).unwrap();

    assert_tokens_preserved(input, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn real_macro_missing_cases_match_golden() {
    let input = include_str!("fixtures/real_macro_missing_cases.rs");
    let expected = include_str!("fixtures/real_macro_missing_cases.expected");
    let actual = rust_fmt_mf::format_source(input, "rustfmt", "2021", None).unwrap();

    assert_tokens_preserved(input, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn real_main_fmt_matches_golden() {
    let input = include_str!("fixtures/real_main_fmt.rs");
    let expected = include_str!("fixtures/real_main_fmt.expected");
    let actual = rust_fmt_mf::format_source(input, "rustfmt", "2021", None).unwrap();

    assert_tokens_preserved(input, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn canonical_macro_body_is_not_degraded() {
    let source = "macro_rules! m {\n    ($fmt:expr, $($arg:expr),+ $(,)?) => {\n        format!($fmt, $($arg),+)\n    };\n}\n";
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();

    assert_eq!(actual, source);
}

#[test]
fn formatting_preserves_unicode_identifiers() {
    let source = r#"
fn привет() {}
macro_rules! m { () => { привет() }; }
"#;
    let output = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &output);
}

#[test]
fn formatting_preserves_string_literal_contents() {
    let source = r#"macro_rules! m { () => { "a . b  c & d :: e" }; }"#;
    let output = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &output);
}

#[test]
fn formatting_preserves_user_marker_identifiers() {
    let source = r#"macro_rules! m { ($x:expr) => { let __m_0 = 10; $x + __m_0 }; }"#;
    let output = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &output);
}

#[test]
fn public_call_is_idempotent_for_compact_macro_headers() {
    let source = include_str!("fixtures/huge_macro.rs");
    let once = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    let twice = rust_fmt_mf::format_source(&once, "rustfmt", "2021", None).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn format_report_lists_each_macro_outcome() {
    let source = "macro_rules! compact{()=>{1};}\n";
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(report.macros.len(), 1);
    assert_eq!(report.macros[0].name, "compact");
    assert!(matches!(
        report.macros[0].status,
        rust_fmt_mf::types::MacroStatus::Formatted
    ));
}

#[test]
fn cli_reports_macro_outcomes_on_stderr() {
    let source = "macro_rules! compact{()=>{1};}\n";
    let mut child = Command::new(env!("CARGO_BIN_EXE_rust-fmt-mf"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "rust-fmt-mf\tFORMATTED\tcompact\t0..30\n"
    );
}

#[test]
fn matcher_line_comments_cannot_swallow_following_tokens() {
    let source = r#"macro_rules! commented_matcher {
    (
        $left:expr, // left operand
        $right:expr $(,)?
    ) => {{ $left + $right }};
}
"#;
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
}

#[test]
fn final_pass_marker_comments_cannot_collide_with_user_comments() {
    let source = r#"/**** __mf_nm_0__ ****/
macro_rules! m { () => { 1 }; }
fn main() {}
"#;
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
    assert_eq!(report.text.matches("__mf_nm_0__").count(), 1);
}

#[test]
fn repetition_marker_identifiers_cannot_collide_with_user_code() {
    let source = r#"macro_rules! m {
    () => { __mf_rep_star! { value } };
}
"#;
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
}

#[test]
fn formatting_never_removes_a_tuple_trailing_comma() {
    let source = r#"macro_rules! one_tuple {
    () => {
        (
            1,
        )
    };
}
"#;
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
}

#[test]
fn non_brace_transcribers_are_formatted_losslessly() {
    let source =
        "macro_rules! paren (($x:expr) => ($x););\nmacro_rules! bracket [($x:expr) => [$x];];\n";
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
    assert_eq!(
        report.text,
        "macro_rules! paren (\n    ($x:expr) => ($x);\n);\n\nmacro_rules! bracket [\n    ($x:expr) => [$x];\n];\n"
    );
    assert_eq!(report.macros.len(), 2);
    assert!(report
        .macros
        .iter()
        .all(|outcome| matches!(outcome.status, rust_fmt_mf::types::MacroStatus::Formatted)));
}

#[test]
fn arbitrary_repetition_separators_are_lossless() {
    let source = "macro_rules! pipes { ($($x:ident)|*) => { $($x)|* }; }\n";
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
}

#[test]
fn nested_block_indentation_is_relative_to_the_macro_body() {
    let source = r#"macro_rules! nested_block {
    () => {
        {
                    let value = 1;
                    value
                }
    };
}
"#;
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert!(report
        .text
        .contains("        {\n            let value = 1;\n            value\n        }"));
}

#[test]
fn nested_repetitions_do_not_keep_shadow_bracket_padding() {
    let source = "macro_rules! nested { ( $( $( $x:expr );* ),+ ) => { vec![ $( vec![ $( $x ),* ] ),+ ] }; }\n";
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert!(report.text.contains("vec![$(vec![$($x),*]),+]"));
}

#[test]
fn rustfmt_cannot_remove_blocks_inside_repetitions() {
    let source = r#"macro_rules! spawn_tasks {
    ($( $name:ident : $body:expr ),+ $(,)?) => {
        $( let handle = std::thread::spawn( move || { $body } ); )+
    };
}
"#;
    let report = rust_fmt_mf::format_source_with_report(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &report.text);
    assert!(!matches!(
        report.macros[0].status,
        rust_fmt_mf::types::MacroStatus::Skipped { .. }
    ));
    assert!(report.text.contains("move || { $body }"));
}
