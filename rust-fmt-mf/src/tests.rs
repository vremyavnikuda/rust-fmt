use crate::mapper::*;
use crate::parser::*;
use crate::replacer::*;
use crate::shadow::*;
use crate::types::Mapping;
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
    assert!(body.contains("hello world"), "body should contain raw string content");
}

#[test]
fn test_multi_macro_with_raw_string() {
    let source = "\nmacro_rules! first {\n    ($x:expr) => {\n        let s = r#\"data\"#;\n        $x\n    };\n}\nmacro_rules! second {\n    () => { 42 };\n}\n";
    let defs = parse_macro_defs(source).unwrap();
    assert!(defs.len() >= 2, "should find at least 2 macros, found {}", defs.len());
}

#[test]
fn test_no_macros() {
    let source = "fn main() { println!(\"hello\"); }";
    let defs = parse_macro_defs(source).unwrap();
    assert_eq!(defs.len(), 0);
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
    let section =
        "    impl __m_0 {\n        __mf_rep_plus! {\n            __m_1\n        }\n    }";
    let mapping = make_mapping(&[("__m_0", "$struct_name"), ("__m_1", "$field")]);
    let result = map_arm_section(section, &mapping);
    assert!(result.contains("$("));
    assert!(result.contains(")+"));
    assert!(result.contains("$struct_name"));
    assert!(result.contains("$field"));
}

#[test]
fn test_split_shadow_into_arms() {
    let shadow = "#![allow(unused_attributes, dead_code)]\n\nmacro_rules! __rustfmt_mf_arm_0 {\n    () => {\n        let x = 1;\n    };\n}\n\nmacro_rules! __rustfmt_mf_arm_1 {\n    () => {\n        let y = 2;\n    };\n}\n";
    let sections = split_shadow_into_arms(shadow);
    assert_eq!(sections.len(), 2);
    assert!(sections[0].contains("let x = 1"));
    assert!(sections[1].contains("let y = 2"));
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
