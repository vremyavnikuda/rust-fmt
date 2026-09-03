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
    assert_only_commas_added(input, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn parenthesized_block_invocation_keeps_block_on_opening_line() {
    let source = r#"macro_rules! run_block {
    ($body:block) => {{
        (move || $body)()
    }};
}

fn main() {
    let _ = run_block!(
        {
            let value = 40;
            value + 2
        }
    );
}
"#;
    let expected = r#"macro_rules! run_block {
    ($body:block) => {{
        (move || $body)()
    }};
}

fn main() {
    let _ = run_block!({
        let value = 40;
        value + 2
    });
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn nested_blocks_do_not_preserve_arbitrary_blank_lines() {
    let source = r#"macro_rules! value {
    () => {
        let value = 42;

        value
    };
}

fn main() {
    let value = value!();

    println!("{}", value);
}
"#;
    let expected = r#"macro_rules! value {
    () => {
        let value = 42;
        value
    };
}

fn main() {
    let value = value!();
    println!("{}", value);
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn statement_repetitions_use_block_indentation() {
    let source = r#"macro_rules! optional_statement {
    ($($value:expr)?) => {
        $(let _ = $value;)?
    };
}
"#;
    let expected = r#"macro_rules! optional_statement {
    ($($value:expr)?) => {
        $(
            let _ = $value;
        )?
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn line_comment_expands_a_compact_matcher() {
    let source = r#"macro_rules! commented {
    ($left:expr, // keep this matcher comment
    $right:expr) => {
        $left + $right
    };
}
"#;
    let expected = r#"macro_rules! commented {
    (
        $left:expr, // keep this matcher comment
        $right:expr
    ) => {
        $left + $right
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn generated_enum_body_expands_from_one_line() {
    let source = r#"macro_rules! define_enum {
    ($name:ident { $($variant:ident),+ }) => {
        pub enum $name { $($variant,)+ }
    };
}
"#;
    let expected = r#"macro_rules! define_enum {
    ($name:ident { $($variant:ident),+ }) => {
        pub enum $name {
            $($variant,)+
        }
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn generated_struct_where_clause_is_whitespace_independent() {
    let source = r#"macro_rules! bounded {
    ($vis:vis $name:ident $param:ident $bound:path $field:ident $ty:ty) => {
        $vis struct $name<$param> where $param: $bound { $(pub $field: $ty),+ }
    };
}
"#;
    let expected = r#"macro_rules! bounded {
    ($vis:vis $name:ident $param:ident $bound:path $field:ident $ty:ty) => {
        $vis struct $name<$param>
        where
            $param: $bound
        {
            $(pub $field: $ty),+
        }
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn partially_expanded_generated_struct_close_brace_is_repaired() {
    let source = r#"macro_rules! bounded {
    ($vis:vis $name:ident $param:ident $bound:path $field:ident $ty:ty) => {
        $vis struct $name<$param>
        where
            $param: $bound
        {
            $(pub $field: $ty),+ }
    };
}
"#;
    let expected = r#"macro_rules! bounded {
    ($vis:vis $name:ident $param:ident $bound:path $field:ident $ty:ty) => {
        $vis struct $name<$param>
        where
            $param: $bound
        {
            $(pub $field: $ty),+
        }
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(actual, expected);
    assert_tokens_preserved(source, &actual);
}

#[test]
fn generated_impl_expands_inline_method_body() {
    let source = r#"macro_rules! implement {
    ($ty:ty, $($trait:ty => $method:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty),+) => {
        $(
            impl $trait for $ty {
                fn $method($($arg: $arg_ty),*) -> $ret { unimplemented!() }
            }
        )+
    };
}
"#;
    let expected = r#"macro_rules! implement {
    ($ty:ty, $($trait:ty => $method:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty),+) => {
        $(
            impl $trait for $ty {
                fn $method($($arg: $arg_ty),*) -> $ret {
                    unimplemented!()
                }
            }
        )+
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn partially_expanded_generated_impl_is_repaired() {
    let source = r#"macro_rules! implement {
    ($ty:ty, $($trait:ty => $method:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty),+) => {
        $(
            impl $trait for $ty { fn $method($($arg: $arg_ty),*) -> $ret {
                unimplemented!()
            }
        }
        )+
    };
}
"#;
    let expected = r#"macro_rules! implement {
    ($ty:ty, $($trait:ty => $method:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty),+) => {
        $(
            impl $trait for $ty {
                fn $method($($arg: $arg_ty),*) -> $ret {
                    unimplemented!()
                }
            }
        )+
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(actual, expected);
    assert_tokens_preserved(source, &actual);
}

#[test]
fn optional_generated_fields_use_nested_indentation() {
    let source = r#"macro_rules! optional_fields {
    ($vis:vis $name:ident $param:ident $bound:path $($field:ident: $ty:ty),+) => {
        $vis struct $name<$param> where
        $param: $bound { $(
        $(pub $field: $ty,)+
        )? }
    };
}
"#;
    let expected = r#"macro_rules! optional_fields {
    ($vis:vis $name:ident $param:ident $bound:path $($field:ident: $ty:ty),+) => {
        $vis struct $name<$param>
        where
            $param: $bound
        {
            $(
                $(pub $field: $ty,)+
            )?
        }
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
    assert_eq!(actual, expected);
}

#[test]
fn generated_where_clause_ignores_const_expression_braces() {
    let source = r#"macro_rules! const_bounded {
    ($vis:vis $name:ident $n:ident $field:ident) => {
        $vis struct $name<const $n: usize> where [(); { $n }]: Sized { pub $field: [u8; $n], }
    };
}
"#;
    let expected = r#"macro_rules! const_bounded {
    ($vis:vis $name:ident $n:ident $field:ident) => {
        $vis struct $name<const $n: usize>
        where
            [(); { $n }]: Sized
        {
            pub $field: [u8; $n],
        }
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_tokens_preserved(source, &actual);
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

#[test]
fn rustfmt_may_add_only_trailing_commas_to_generated_struct_fields() {
    let source = r#"macro_rules! borrowed_type {
    ($name:ident, $lt:lifetime, $ty:ty) => {
        pub struct $name<$lt> {
            pub value: &$lt $ty
        }
    };
}

macro_rules! pass_item {
    ($item:item) => {
        $item
    };
}

pass_item! {
    pub struct Generated {
        pub value: i32
    }
}
"#;
    let expected = r#"macro_rules! borrowed_type {
    ($name:ident, $lt:lifetime, $ty:ty) => {
        pub struct $name<$lt> {
            pub value: &$lt $ty,
        }
    };
}

macro_rules! pass_item {
    ($item:item) => {
        $item
    };
}

pass_item! {
    pub struct Generated {
        pub value: i32,
    }
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(actual, expected);
    assert_only_commas_added(source, &actual);
}

#[test]
fn nested_generated_macro_rules_is_indented_in_one_public_call() {
    let source = r#"macro_rules! make_tripler {
    ($d:tt $name:ident) => {
        macro_rules!      $name{($d value:expr)=>{$d           value*3};}
    };
}
"#;
    let expected = r#"macro_rules! make_tripler {
    ($d:tt $name:ident) => {
        macro_rules! $name {
            ($d value:expr) => {
                $d value * 3
            };
        }
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn long_macro_matcher_is_wrapped_below_the_style_width() {
    let source = r#"macro_rules! huge {
    ($(#[$attr:meta])* $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty $body:block) => {
        $(#[$attr])*
        $vis fn $name($($arg: $ty),*) -> $ret $body
    };
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(actual.contains("macro_rules! huge {\n    (\n"), "{actual}");
    assert!(
        actual.lines().all(|line| line.chars().count() <= 100),
        "{actual}"
    );
    assert_only_commas_added(source, &actual);
}

#[test]
fn long_simple_macro_argument_list_uses_greedy_line_packing() {
    // A user-defined macro's invocation arguments should wrap the same way
    // rustfmt wraps a builtin call like `vec![1, 2, 3, ...]`: pack as many
    // items per line as fit, not one item per line.
    let source = r#"macro_rules! values {
    ($($value:expr),+ $(,)?) => {
        ($($value),+)
    };
}

fn main() {
    let _ = values!(
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30
    );
}
"#;
    let expected_call = r#"    let _ = values!(
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30
    );"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(actual.contains(expected_call), "{actual}");
    assert!(
        actual.lines().all(|line| line.chars().count() <= 100),
        "{actual}"
    );
    assert_tokens_preserved(source, &actual);
}

#[test]
fn macro_invocation_width_accounts_for_the_trailing_semicolon() {
    let source = r#"macro_rules! field_accessor {
    ($name:ident, $($field:ident: $ty:ty),+ $(,)?) => {};
}

field_accessor!(DataFields, name: String, age: u32, email: String, active: bool);
"#;
    // Short argument lists that don't fit on the invocation's own line still
    // fit on one wrapped line once indented — rustfmt would not explode a
    // list this short across five lines.
    let expected_call = r#"field_accessor!(
    DataFields, name: String, age: u32, email: String, active: bool
);"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(actual.contains(expected_call), "{actual}");
    assert!(actual.lines().all(|line| line.chars().count() <= 100));
    assert_tokens_preserved(source, &actual);
}

#[test]
fn dense_but_short_method_chain_stays_on_one_line() {
    // Regression test: rust-fmt-mf used to hard-code max_width=80 /
    // chain_width=40 for the non-macro rustfmt pass, which is narrower than
    // rustfmt's real defaults (100 / 60) and broke chains that any
    // `cargo fmt` user would expect to stay on one line.
    let source = r#"pub fn macro_repetition_in_fn() {
    let v: Vec<i32> = vec![0; 100];
    let sum: i32 = v.iter().map(|x| x * 2).filter(|x| x % 3 == 0).sum();
    println!("sum = {}", sum);
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(
        actual
            .contains("    let sum: i32 = v.iter().map(|x| x * 2).filter(|x| x % 3 == 0).sum();\n"),
        "{actual}"
    );
}

#[test]
fn layout_normalization_separates_items_but_not_list_entries() {
    let source = r#"pub mod nested {
    pub struct Item {
        pub first: i32,

        pub second: i32,
    }
    impl Item {
        pub fn first(&self) -> i32 {
            self.first
        }
        pub fn second(&self) -> i32 {
            self.second
        }
    }
}
"#;
    let expected = r#"pub mod nested {
    pub struct Item {
        pub first: i32,
        pub second: i32,
    }

    impl Item {
        pub fn first(&self) -> i32 {
            self.first
        }

        pub fn second(&self) -> i32 {
            self.second
        }
    }
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(actual, expected);
    assert_tokens_preserved(source, &actual);
}

#[test]
fn consecutive_module_declarations_stay_compact() {
    let source = "pub mod first;\n\npub mod second;\n\nmod third;\n";
    let expected = "pub mod first;\npub mod second;\nmod third;\n";
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn default_style_wraps_long_signatures_and_dense_method_chains() {
    // These stay at rustfmt's real default width (100 / chain 60), not the
    // narrower 80/40 rust-fmt-mf used to hard-code, so the identifiers here
    // are deliberately long enough to still exceed 100 columns and need
    // wrapping either way — verified directly against `rustfmt` itself.
    let source = r#"pub fn long_signature(alpha: i32, bravo: i32, charlie: i32, delta: i32, echo: i32, foxtrot: i32, golf: i32) -> i32 { alpha + bravo + charlie + delta + echo + foxtrot + golf }

pub fn chain(values: &[i32]) -> i32 { values.iter().map(|value| value * 2).filter(|value| value % 3 == 0).sum() }
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(actual.contains("pub fn long_signature(\n"), "{actual}");
    assert!(
        actual.contains("values\n        .iter()\n        .map("),
        "{actual}"
    );
    assert_only_commas_added(source, &actual);
}

#[test]
fn short_signatures_and_chains_stay_on_one_line_like_rustfmt() {
    // Regression test for the hard-coded max_width=80 / chain_width=40 bug:
    // signatures and chains that comfortably fit under rustfmt's real
    // default width (100 / chain 60) must not be wrapped.
    let source = r#"pub fn long_signature(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) -> i32 { a + b + c + d + e + f + g }

pub fn sort(items: &mut [f64]) {
    items.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(
        actual.contains(
            "pub fn long_signature(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) -> i32 {\n"
        ),
        "{actual}"
    );
    assert!(
        actual.contains(
            "items.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));\n"
        ),
        "{actual}"
    );
    assert_only_commas_added(source, &actual);
}

#[test]
fn narrow_style_accepts_rustfmt_closure_block_braces() {
    let source = r#"pub fn sort(items: &mut [f64]) {
    items.sort_by(|first_value, second_value| first_value.partial_cmp(second_value).unwrap_or(std::cmp::Ordering::Equal));
}
"#;
    let actual = rust_fmt_mf::format_source(source, "rustfmt", "2021", None).unwrap();
    assert!(
        actual.contains("items.sort_by(|first_value, second_value| {\n"),
        "{actual}"
    );
    assert_only_layout_tokens_added(source, &actual);
}

fn assert_only_commas_added(input: &str, output: &str) {
    assert_only_tokens_added(input, output, &[","]);
}

fn assert_only_layout_tokens_added(input: &str, output: &str) {
    assert_only_tokens_added(input, output, &[",", "{", "}"]);
}

fn assert_only_tokens_added(input: &str, output: &str, allowed: &[&str]) {
    let before = rust_fmt_mf::parser::significant_tokens(input).unwrap();
    let after = rust_fmt_mf::parser::significant_tokens(output).unwrap();
    let mut left = 0usize;
    let mut right = 0usize;
    while left < before.len() && right < after.len() {
        if before[left].kind == after[right].kind && before[left].text == after[right].text {
            left += 1;
            right += 1;
        } else {
            assert!(
                allowed.contains(&after[right].text.as_str()),
                "unexpected token change"
            );
            right += 1;
        }
    }
    assert_eq!(left, before.len(), "formatter removed input tokens");
    assert!(
        after[right..]
            .iter()
            .all(|token| allowed.contains(&token.text.as_str())),
        "formatter added a disallowed token"
    );
}
