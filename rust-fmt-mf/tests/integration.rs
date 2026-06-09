use rust_fmt_mf::formatter::run_rustfmt;
use rust_fmt_mf::parser::parse_macro_defs;
use rust_fmt_mf::replacer::replace_macro_syntax;
use rust_fmt_mf::shadow::build_shadow_file;
use rust_fmt_mf::types::Mapping;

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
    // Multi-line body should force opener onto its own line
    assert!(
        result.starts_with("define_enum!(\n"),
        "Expected opener on its own line, got:\n{result}"
    );
    // Internal spacing of body content is preserved (DSL opaque)
    assert!(
        result.contains("MyGeneratedEnum  {"),
        "Expected preserved double space in body"
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
