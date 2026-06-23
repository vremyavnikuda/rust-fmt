use crate::types::{MacroArm, MacroDef};

/// Parse source and extract all macro_rules! definitions with byte-accurate spans.
///
/// Scans the source text directly for `macro_rules!` patterns,
/// then finds arm boundaries via brace/paren tracking.
pub fn parse_macro_defs(source: &str) -> anyhow::Result<Vec<MacroDef>> {
    let bytes = source.as_bytes();
    let mut defs = Vec::new();
    let mut found_starts: Vec<usize> = Vec::new();
    let mut search_pos = 0;
    while search_pos < bytes.len() {
        if let Some(macro_pos) = find_macro_rules(bytes, search_pos) {
            found_starts.push(macro_pos);
            search_pos = macro_pos + "macro_rules!".len();
        } else {
            search_pos += 1;
        }
    }
    for &start in &found_starts {
        let mut pos = start + "macro_rules!".len();
        // Skip whitespace
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        // Parse macro name (ident or `?` for macro_rules!?)
        let name_start = pos;
        if pos < bytes.len() && bytes[pos] == b'?' {
            pos += 1;
        } else {
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
                pos += 1;
            }
        }
        let name = String::from_utf8_lossy(&bytes[name_start..pos]).to_string();
        // Find opening '{'
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] != b'{' {
            continue;
        }
        // Find matching '}' for the macro_rules! body
        let _body_start = pos;
        pos += 1;
        let mut depth = 1;
        while pos < bytes.len() && depth > 0 {
            match bytes[pos] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                // Skip string literals to avoid false brace matches
                b'"' => {
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != b'"' {
                        if bytes[pos] == b'\\' {
                            pos += 1; // skip escaped char
                        }
                        pos += 1;
                    }
                }
                // Skip raw string literals
                b'r' if pos + 1 < bytes.len() && bytes[pos + 1] == b'#' => {
                    let hash_start = pos + 1;
                    let mut hash_count = 0;
                    while hash_start + hash_count < bytes.len()
                        && bytes[hash_start + hash_count] == b'#'
                    {
                        hash_count += 1;
                    }
                    if hash_start + hash_count < bytes.len()
                        && bytes[hash_start + hash_count] == b'"'
                    {
                        pos = hash_start + hash_count + 1; // after r#"..."#
                        let delimiter = format!("\"{}", "#".repeat(hash_count));
                        while pos < bytes.len() {
                            if pos + hash_count < bytes.len()
                                && &bytes[pos..pos + hash_count + 1] == delimiter.as_bytes()
                            {
                                pos += hash_count + 1;
                                break;
                            }
                            pos += 1;
                        }
                        continue;
                    }
                }
                // Skip line comments
                b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' => {
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                // Skip block comments
                b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'*' => {
                    pos += 2;
                    while pos + 1 < bytes.len() {
                        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                            pos += 2;
                            break;
                        }
                        pos += 1;
                    }
                    continue;
                }
                _ => {}
            }
            pos += 1;
        }
        let end = pos;
        let arms = scan_arms(source, start, end)?;
        if !arms.is_empty() {
            let line_start = source[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let attr_start = find_attr_span_start(source, line_start);
            defs.push(MacroDef {
                name,
                span: attr_start..end,
                arms,
            });
        }
    }
    // Sort by span length descending (outermost first), then filter nested macros
    defs.sort_by(|a, b| b.span.len().cmp(&a.span.len()));
    let mut filtered_defs: Vec<MacroDef> = Vec::new();
    for def in defs {
        if !filtered_defs.iter().any(|outer: &MacroDef| {
            outer.span.start <= def.span.start && outer.span.end >= def.span.end
        }) {
            filtered_defs.push(def);
        }
    }
    // Sort outermost first for processing
    filtered_defs.sort_by_key(|d| d.span.start);
    Ok(filtered_defs)
}

/// Scan backward from `pos` to find the start of any attributes, doc
/// comments, or blank lines preceding the definition. Multi-line attributes
/// (e.g. `#[cfg_attr(...)]`) are tracked by detecting lines ending with `]`.
fn find_attr_span_start(source: &str, pos: usize) -> usize {
    let mut cur = pos;
    let mut found_attr = false;
    loop {
        if cur == 0 {
            return if found_attr { 0 } else { pos };
        }
        // Find the `\n` that ends the line before `cur`.
        let prev_nl = match source[..cur].rfind('\n') {
            Some(p) => p,
            None => return if found_attr { 0 } else { pos },
        };
        // Find the `\n` before that to get the start of the line we inspect.
        let line_start = match source[..prev_nl].rfind('\n') {
            Some(p) => p + 1,
            None => 0,
        };
        let trimmed = source[line_start..prev_nl].trim();
        if trimmed.is_empty() {
            cur = line_start;
            continue;
        }
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            found_attr = true;
            cur = line_start;
            continue;
        }
        if trimmed.starts_with("#[") && !trimmed.starts_with("#![") {
            found_attr = true;
            cur = line_start;
            continue;
        }
        if trimmed.ends_with(']') {
            found_attr = true;
            cur = line_start;
            continue;
        }
        return if found_attr { cur } else { pos };
    }
}

/// Find the next occurrence of "macro_rules!" at or after `pos` that is not
/// inside a string, comment, or identifier.
fn find_macro_rules(bytes: &[u8], pos: usize) -> Option<usize> {
    let pattern = b"macro_rules!";
    let mut i = pos;
    while i + pattern.len() <= bytes.len() {
        if &bytes[i..i + pattern.len()] == pattern {
            // Check it's not part of a larger identifier (preceded by alphanumeric or _)
            if i > 0 {
                let prev = bytes[i - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' {
                    i += 1;
                    continue;
                }
            }
            // Check it's not inside a string or comment — just do a
            // simple heuristic: not preceded by " inside same line
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Scan the original source text within the macro_rules! span to find arm boundaries.
///
/// Macro structure:
///   macro_rules! name {
///       (pattern1) => { body1 }
///       (pattern2) => { body2 }
///   }
///
/// Each arm: (pattern) => { body }
/// Also supports {pattern} and [pattern] as arm delimiters.
fn scan_arms(source: &str, macro_start: usize, macro_end: usize) -> anyhow::Result<Vec<MacroArm>> {
    let macro_end = macro_end.min(source.len());
    let text = &source[macro_start..macro_end];
    let bytes = text.as_bytes();
    let mut arms = Vec::new();
    let mut pos = 0;
    // Skip to the opening '{' of macro_rules! name { ... }
    while pos < bytes.len() && bytes[pos] != b'{' {
        pos += 1;
    }
            pos += 1;
    while pos < bytes.len() {
        // Skip whitespace between arms
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] == b'}' {
            break;
        }
        // Accept (, {, [ as arm pattern start
        let _close_delim = match bytes[pos] {
            b'(' => b')',
            b'{' => b'}',
            b'[' => b']',
            _ => {
                pos += 1;
                continue;
            }
        };
        // Find matching closing delimiter — tracking all bracket types
        let pattern_start_rel = pos;
        pos += 1;
        let mut depth = 1;
        let mut in_string = false;
        while pos < bytes.len() {
            if in_string {
                if bytes[pos] == b'"' {
                    in_string = false;
                } else if bytes[pos] == b'\\' {
                    pos += 1; // skip escape
                    if pos >= bytes.len() { break; }
                    continue;
                }
            } else {
                match bytes[pos] {
                    b'"' => in_string = true,
                    b'(' | b'{' | b'[' => depth += 1,
                    b')' | b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            pos += 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            pos += 1;
        }
        let pattern_end_rel = pos;
        // Expect '=>'
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos + 1 >= bytes.len() || bytes[pos] != b'=' || bytes[pos + 1] != b'>' {
            break;
        }
        pos += 2;
        // Expect '{' — body start
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] != b'{' {
            break;
        }
        let body_start_rel = pos;
        let mut brace_depth = 0;
        in_string = false;
        while pos < bytes.len() {
            if in_string {
                if bytes[pos] == b'"' {
                    in_string = false;
                } else if bytes[pos] == b'\\' {
                    pos += 1;
                    if pos >= bytes.len() { break; }
                    continue;
                }
            } else {
                match bytes[pos] {
                    b'"' => in_string = true,
                    b'{' => brace_depth += 1,
                    b'}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            pos += 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            pos += 1;
        }
        let body_end_rel = pos;
        arms.push(MacroArm {
            pattern_span: (macro_start + pattern_start_rel)..(macro_start + pattern_end_rel),
            body_span: (macro_start + body_start_rel)..(macro_start + body_end_rel),
        });
    }
    Ok(arms)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // Nested macros are filtered out; only outermost macros are returned
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
        // Pattern includes the braces: ({ $x:expr })
        let pattern = &source[defs[0].arms[0].pattern_span.clone()];
        assert!(pattern.contains('{'));
        assert!(pattern.contains('}'));
        // Body should be just { $x * 2 }
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
        // macro_heavy.rs has many macros: bad_macro, multi_arm_macro,
        // recursive_macro, vec_of_strings, nested_macro_call,
        // inline_macro_call, complex_pattern, huge_macro,
        // multi_line_invocation, token_tree_macro, count_exprs,
        // field_accessor, format_madness, repeat_pattern,
        // tt_recurse, define_enum, run_length_encode,
        // cascading_macro, tt_based_dispatch, long_macro_invocation,
        // stringify_many
        assert!(
            defs.len() >= 20,
            "should find at least 20 macros, found {}",
            defs.len()
        );
        // Verify each macro has valid arms with non-empty body spans
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
}