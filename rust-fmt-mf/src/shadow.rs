use proc_macro2::TokenStream;

/// Build a shadow file containing all arms as macro_rules! definitions.
///
/// Uses `macro_rules! __rustfmt_mf_arm_N { () => { body }; }` wrappers
/// which are valid in all Rust contexts (unlike `mod` which rejects `let`
/// and expressions, or `const` which rejects `impl`/`struct` items).
/// The dummy macros for repetition markers are also defined so that
/// `__mf_rep_*! { }` invocations work in all contexts.
///
/// Returns: (shadow_file_text, number_of_arms)
pub fn build_shadow_file(replaced_bodies: &[TokenStream]) -> (String, usize) {
    let arm_count = replaced_bodies.len();
    let mut modules = TokenStream::new();
    for (i, body) in replaced_bodies.iter().enumerate() {
        let macro_name = quote::format_ident!("__rustfmt_mf_arm_{}", i);
        modules.extend(quote::quote! {
            macro_rules! #macro_name {
                () => { #body };
            }
        });
    }
    let file = quote::quote! {
        #![allow(unused_attributes, dead_code, unused_variables, unused_macros, unused_braces)]
        macro_rules! __mf_rep_star { ($($t:tt)*) => { $($t)* }; }
        macro_rules! __mf_rep_plus { ($($t:tt)*) => { $($t)* }; }
        macro_rules! __mf_rep_question { ($($t:tt)*) => { $($t)* }; }
        macro_rules! __mf_rep_star_comma { ($($t:tt)*) => { $($t)* }; }
        macro_rules! __mf_rep_plus_comma { ($($t:tt)*) => { $($t)* }; }
        macro_rules! __mf_rep_star_semi { ($($t:tt)*) => { $($t)* }; }
        macro_rules! __mf_rep_plus_semi { ($($t:tt)*) => { $($t)* }; }
        #modules
    };
    (file.to_string(), arm_count)
}

/// Build a shadow file from pre-formatted body strings.
///
/// Unlike `build_shadow_file` which uses TokenStream (losing multi-line
/// formatting), this function accepts already-formatted body strings
/// and embeds them directly into the shadow text, preserving newlines.
///
/// Each body string MUST have 4-space relative indentation for each level.
/// rustfmt will adjust the absolute indentation to match the arm context.
pub fn build_shadow_file_from_strings(body_strings: &[String]) -> String {
    let mut s = String::new();
    s.push_str("#![allow(unused_attributes, dead_code, unused_variables, unused_macros, unused_braces)]\n\n");
    for kind in &[
        "star",
        "plus",
        "question",
        "star_comma",
        "plus_comma",
        "star_semi",
        "plus_semi",
    ] {
        s.push_str(&format!(
            "macro_rules! __mf_rep_{kind} {{\n ($($t:tt)*) => {{ $($t)* }};\n}}\n",
            kind = kind
        ));
    }
    s.push('\n');
    for (i, body) in body_strings.iter().enumerate() {
        // Normalize to 0-indent baseline so rustfmt's non-uniform body-indent
        // shift (first line→body_indent, others→body_indent+original) preserves
        // relative indentation.
        let lines: Vec<&str> = body.lines().collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        let indented = lines
            .iter()
            .map(|l| {
                if l.trim().is_empty() {
                    String::new()
                } else {
                    let line_indent = l.len() - l.trim_start().len();
                    let rel_indent = line_indent.saturating_sub(min_indent);
                    let trimmed = l.trim();
                    let mut line = String::with_capacity(rel_indent + trimmed.len());
                    line.push_str(&" ".repeat(rel_indent));
                    line.push_str(trimmed);
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        s.push_str(&format!(
            "macro_rules! __rustfmt_mf_arm_{i} {{\n () => {{\n{}\n }};\n}}\n\n",
            indented
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
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
}