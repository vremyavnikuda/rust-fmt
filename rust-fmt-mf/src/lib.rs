pub mod formatter;
pub mod mapper;
pub mod parser;
pub mod replacer;
pub mod shadow;
pub mod types;

use crate::formatter::{run_rustfmt, run_rustfmt_no_macro};
use crate::mapper::apply_formatting;
use crate::parser::parse_macro_defs;
use crate::replacer::replace_macro_syntax_text;
use crate::shadow::build_shadow_file_from_strings;
use crate::types::Mapping;

fn try_format_as_mod(
    inner: &str,
    id: usize,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> Option<String> {
    let wrapper = format!("mod __mf_rep_{id} {{\n{inner}\n}}");
    let formatted = run_rustfmt(&wrapper, rustfmt_path, edition, config_path).ok()?;
    extract_wrapper_body(&formatted, "mod")
}

fn try_format_as_fn(
    inner: &str,
    id: usize,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> Option<String> {
    let wrapper = format!("fn __mf_rep_{id}() {{\n{inner}\n}}");
    let formatted = run_rustfmt(&wrapper, rustfmt_path, edition, config_path).ok()?;
    extract_wrapper_body(&formatted, "fn")
}

fn extract_wrapper_body(formatted: &str, kind: &str) -> Option<String> {
    let lines: Vec<&str> = formatted.lines().collect();
    if lines.len() >= 3 {
        let body: String = lines[1..lines.len() - 1]
            .iter()
            .map(|l| l.strip_prefix("    ").unwrap_or(l))
            .collect::<Vec<_>>()
            .join("\n");
        Some(body)
    } else if lines.len() == 1 {
        let line = lines[0].trim();
        let after = line.find(&format!("{} __mf_rep_", kind))?;
        let rest = &line[after..];
        let brace_start = rest.find('{')?;
        let brace_end = rest.rfind('}')?;
        if brace_start + 1 < brace_end {
            Some(rest[brace_start + 1..brace_end].trim().to_string())
        } else {
            None
        }
    } else {
        None
    }
}

struct RepMarker {
    inner_start: usize,
    inner_end: usize,
    rep_id: usize,
}

fn find_rep_markers(body_str: &str) -> Vec<RepMarker> {
    let bytes = body_str.as_bytes();
    let mut markers = Vec::new();
    let mut i = 0;
    let mut rep_id = 0;
    while i < bytes.len() {
        if body_str[i..].starts_with("__mf_rep_") {
            let kind_start = i + "__mf_rep_".len();
            let rest = &body_str[kind_start..];
            let kind_end = match rest.find('!') {
                Some(pos) => kind_start + pos,
                None => {
                    i += 1;
                    continue;
                }
            };
            let after_kind = &body_str[kind_end..];
            let brace_pos = match after_kind.find('{') {
                Some(pos) => kind_end + pos,
                None => {
                    i += 1;
                    continue;
                }
            };
            let mut depth = 1u32;
            let mut close_pos = brace_pos + 1;
            while close_pos < bytes.len() && depth > 0 {
                if bytes[close_pos] == b'{' {
                    depth += 1;
                }
                if bytes[close_pos] == b'}' {
                    depth -= 1;
                }
                close_pos += 1;
            }
            if depth != 0 {
                i += 1;
                continue;
            }
            markers.push(RepMarker {
                inner_start: brace_pos + 1,
                inner_end: close_pos - 1,
                rep_id,
            });
            rep_id += 1;
            i = close_pos;
        } else {
            i += 1;
        }
    }
    markers
}
fn preformat_rep_bodies(
    body_str: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> String {
    let markers = find_rep_markers(body_str);
    if markers.is_empty() {
        return body_str.to_string();
    }
    let mut result = body_str.to_string();
    for m in markers.into_iter().rev() {
        let inner = result[m.inner_start..m.inner_end].to_string();
        let formatted = try_format_as_mod(&inner, m.rep_id, rustfmt_path, edition, config_path)
            .or_else(|| try_format_as_fn(&inner, m.rep_id, rustfmt_path, edition, config_path));
        if let Some(fmt) = formatted {
            if fmt.contains('\n') {
                result.replace_range(m.inner_start..m.inner_end, &format!("{}\n", fmt));
            } else {
                result.replace_range(m.inner_start..m.inner_end, &fmt);
            }
        }
    }
    result
}
fn body_line_is_attr(trimmed: &str) -> bool {
    let t = trimmed.trim_start();
    t.starts_with("#[") || t.starts_with("#!")
}
fn normalize_body_indent(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let non_empty: Vec<&str> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect();
    if non_empty.is_empty() {
        return body.to_string();
    }
    let min_indent = non_empty
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let has_closer = non_empty
        .iter()
        .any(|l| l.trim() == "}" && (l.len() - l.trim_start().len()) == min_indent);
    let mut result_lines: Vec<String> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                let trimmed = l.trim();
                let indent = l.len() - trimmed.len();
                let effective = if has_closer && trimmed.ends_with('{') {
                    min_indent
                } else {
                    indent
                };
                let rel = if effective > min_indent {
                    effective - min_indent
                } else {
                    0
                };
                format!("{}{}", " ".repeat(4 + rel), trimmed)
            }
        })
        .collect::<Vec<_>>();
    // Post-process: align attribute lines (`#[...]`) that are deeper
    // than the next structural line to the same level.
    // This fixes cases like:
    //     #[$meta]       ← deeper than the struct it decorates
    //     $vis struct ...
    // They should be at the same indent.
    let mut i = 0;
    while i < result_lines.len() {
        let trimmed = result_lines[i].trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        let is_attr = body_line_is_attr(trimmed);
        if !is_attr {
            i += 1;
            continue;
        }
        // Find the next non-empty, non-attr line
        let mut j = i + 1;
        while j < result_lines.len() {
            let next = result_lines[j].trim();
            if next.is_empty() || body_line_is_attr(next) {
                j += 1;
                continue;
            }
            break;
        }
        if j < result_lines.len() {
            let attr_indent = result_lines[i].len() - result_lines[i].trim_start().len();
            let next_indent = result_lines[j].len() - result_lines[j].trim_start().len();
            // If the attr is deeper than the next structural line, align it down
            if attr_indent > next_indent {
                result_lines[i] = format!("{}{}", " ".repeat(next_indent), trimmed);
            }
        }
        i = j;
    }
    // Post-process: align `where` lines that are deeper than the
    // preceding structural line (e.g. the struct/fn/trait it modifies).
    i = 0;
    while i < result_lines.len() {
        let trimmed = result_lines[i].trim();
        if trimmed.is_empty() || !trimmed.starts_with("where") {
            i += 1;
            continue;
        }
        // Find the preceding non-comment, non-attr structural line
        let mut j = if i > 0 { i - 1 } else { 0 };
        while j > 0 {
            let prev = result_lines[j].trim();
            if !prev.is_empty() && !body_line_is_attr(prev) {
                break;
            }
            j -= 1;
        }
        if j < i {
            let where_indent = result_lines[i].len() - result_lines[i].trim_start().len();
            let prev_indent = result_lines[j].len() - result_lines[j].trim_start().len();
            if where_indent > prev_indent + 4 {
                result_lines[i] = format!("{}{}", " ".repeat(prev_indent), "where");
            }
        }
        i += 1;
    }
    // Post-process: align standalone `}` with the nearest preceding
    // standalone `{` when the closer is significantly deeper.
    let mut brace_open_indent: Option<usize> = None;
    i = 0;
    while i < result_lines.len() {
        let trimmed = result_lines[i].trim();
        if trimmed == "{" {
            brace_open_indent = Some(result_lines[i].len() - result_lines[i].trim_start().len());
        } else if trimmed == "}" {
            if let Some(open_indent) = brace_open_indent {
                let close_indent = result_lines[i].len() - result_lines[i].trim_start().len();
                if close_indent > open_indent + 4 {
                    result_lines[i] = format!("{}{}", " ".repeat(open_indent), "}");
                }
            }
            brace_open_indent = None;
        }
        i += 1;
    }
    // Post-process: re-indent lines inside standalone `{`...`}` pairs.
    // If content between `{` and `}` has significantly deeper indent than
    // `{_indent + 4`, bring it down to `{_indent + 4`.
    i = 0;
    while i < result_lines.len() {
        let trimmed = result_lines[i].trim();
        if trimmed != "{" {
            i += 1;
            continue;
        }
        let open_indent = result_lines[i].len() - result_lines[i].trim_start().len();
        let target_indent = open_indent + 4;
        let mut j = i + 1;
        let mut found_close = false;
        while j < result_lines.len() {
            if result_lines[j].trim() == "}" {
                found_close = true;
                break;
            }
            j += 1;
        }
        if found_close {
            for k in (i + 1)..j {
                let line = &result_lines[k];
                let trimmed_line = line.trim();
                if trimmed_line.is_empty() {
                    continue;
                }
                let cur_indent = line.len() - trimmed_line.len();
                if cur_indent > target_indent + 4 {
                    result_lines[k] = format!("{}{}", " ".repeat(target_indent), trimmed_line);
                }
            }
        }
        i = j + 1;
    }
    result_lines.join("\n")
}
/// Re-indent the body content inside macro invocations `ident!(...)`, `ident!{...}`, `ident![...]`.
///
/// rustfmt does not deeply format macro invocation bodies (they may use DSL syntax
/// that isn't valid Rust). We at least fix the indentation for readability.
fn reindent_invocation_bodies(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        // Look for `ident!(` / `ident!{` / `ident![` with optional whitespace between ! and delimiter
        if i + 2 < bytes.len()
            && bytes[i + 1] == b'!'
            && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_')
        {
            // Check for opener after optional whitespace
            let mut opener_pos = i + 2;
            while opener_pos < bytes.len() && bytes[opener_pos] == b' ' {
                opener_pos += 1;
            }
            if opener_pos < bytes.len()
                && (bytes[opener_pos] == b'('
                    || bytes[opener_pos] == b'{'
                    || bytes[opener_pos] == b'[')
            {
                let ident_start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                result.push_str(&text[ident_start..i]);
                if i < bytes.len() && bytes[i] == b'!' {
                    result.push('!');
                    i += 1;
                }
                // Preserve whitespace between ! and opener
                while i < bytes.len() && bytes[i] == b' ' {
                    result.push(' ');
                    i += 1;
                }
                let open = bytes[i];
                let close: u8 = match open {
                    b'(' => b')',
                    b'{' => b'}',
                    _ => b']',
                };
                result.push(open as char);
                i += 1;
                // Find matching close
                let body_start = i;
                let mut depth = 1u32;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == open {
                        depth += 1;
                    } else if bytes[i] == close {
                        depth -= 1;
                    } else if bytes[i] == b'"' {
                        i += 1;
                        while i < bytes.len() && bytes[i] != b'"' {
                            if bytes[i] == b'\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                    }
                    i += 1;
                }
                let body_end = if depth == 0 { i - 1 } else { i };
                let body = &text[body_start..body_end];
                // Only re-indent multi-line bodies
                if !body.contains('\n') {
                    result.push_str(body);
                    result.push(close as char);
                    continue;
                }
                // Compute base indent of the invocation line
                let line_start = text[..body_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
                let inv_line = &text[line_start..body_start];
                let base_indent = inv_line.len() - inv_line.trim_start().len();
                let lines: Vec<&str> = body.lines().collect();
                // If body is multi-line, force opener onto its own line
                // even when the first content starts with spaces (not \n).
                // This fixes cases like `ident!(    content { ... } )`
                // where the body is clearly multi-line but lacks a leading newline.
                let body_starts_with_nl = body.starts_with('\n');
                let body_is_multi_line = !body_starts_with_nl && body.contains('\n');
                // Find last non-empty line index (to avoid trailing blank lines)
                let last_content_idx = lines
                    .iter()
                    .rposition(|l| !l.trim().is_empty())
                    .unwrap_or(lines.len().saturating_sub(1));
                let mut formatted = String::new();
                let mut brace_depth = 0u32;
                let mut is_first_content = true;
                let mut in_where = false;
                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        if idx > 0 && idx < last_content_idx {
                            formatted.push('\n');
                        }
                        continue;
                    }
                    let open_count = trimmed.matches('{').count() as u32;
                    let close_count = trimmed.matches('}').count() as u32;
                    let depth_before = brace_depth;
                    // Determine indent level:
                    //   Line starting with `}` → depth AFTER closing (aligns `}` with matching `{`)
                    //   Line starting with `{` → depth BEFORE opening (`{` stays at current level)
                    //   Other lines              → depth at start of line
                    let indent_depth = if trimmed.starts_with('}') {
                        depth_before.saturating_sub(close_count)
                    } else if trimmed.starts_with('{') {
                        depth_before
                    } else {
                        depth_before
                    };
                    brace_depth = depth_before + open_count - close_count;
                    // `where` clause: lines after `where` get +1 indent until `{`
                    if trimmed == "where" {
                        in_where = true;
                    }
                    let extra_where = if in_where
                        && trimmed != "where"
                        && !trimmed.starts_with('{')
                        && !trimmed.starts_with('}')
                    {
                        1
                    } else {
                        0
                    };
                    if trimmed.starts_with('{') {
                        in_where = false;
                    }
                    let indent = base_indent + 4 + (indent_depth as usize) * 4 + extra_where * 4;
                    if is_first_content {
                        if body_starts_with_nl || body_is_multi_line {
                            formatted.push('\n');
                            formatted.push_str(&" ".repeat(indent));
                        } else {
                            formatted.push_str(&" ".repeat(indent));
                        }
                        formatted.push_str(trimmed);
                        is_first_content = false;
                    } else {
                        formatted.push('\n');
                        formatted.push_str(&" ".repeat(indent));
                        formatted.push_str(trimmed);
                    }
                }
                // If the body is multi-line, emit the close delimiter on its own line
                // at base_indent (same level as the opening `ident!(`).
                let has_line_break = body.contains('\n');
                if has_line_break {
                    formatted.push('\n');
                    formatted.push_str(&" ".repeat(base_indent));
                }
                result.push_str(&formatted);
                result.push(close as char);
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}
fn final_format_pass(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let macro_defs = parse_macro_defs(source)?;
    if macro_defs.is_empty() {
        let formatted = run_rustfmt_no_macro(source, rustfmt_path, edition, config_path)?;
        return Ok(reindent_invocation_bodies(&formatted));
    }
    let mut text = source.to_string();
    for (i, def) in macro_defs.iter().enumerate().rev() {
        let placeholder = format!("/**** __mf_nm_{i}__ ****/");
        text.replace_range(def.span.clone(), &placeholder);
    }
    let formatted = run_rustfmt_no_macro(&text, rustfmt_path, edition, config_path)?;
    let mut result = formatted;
    for (i, def) in macro_defs.iter().enumerate() {
        let placeholder = format!("/**** __mf_nm_{i}__ ****/");
        let orig_def = &source[def.span.clone()];
        result = result.replacen(&placeholder, orig_def, 1);
    }
    result = reindent_invocation_bodies(&result);
    Ok(result)
}
pub fn format_source(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let macro_defs = parse_macro_defs(source)?;
    if macro_defs.is_empty() {
        return final_format_pass(source, rustfmt_path, edition, config_path);
    }
    let mut all_replaced_bodies_str: Vec<String> = Vec::new();
    let mut all_mappings: Vec<Mapping> = Vec::new();
    for def in &macro_defs {
        for arm in &def.arms {
            let arm_body_text = &source[arm.body_span.clone()];
            let body_text = arm_body_text.trim();
            let inner_text = if body_text.starts_with('{') && body_text.ends_with('}') {
                let inner = &body_text[1..body_text.len() - 1];
                let trimmed = inner.strip_prefix('\n').unwrap_or(inner);
                let trimmed = trimmed.strip_suffix('\n').unwrap_or(trimmed);
                trimmed.to_string()
            } else {
                body_text.to_string()
            };
            let mut mapping = Mapping::new();
            let mut inner_str = replace_macro_syntax_text(&inner_text, &mut mapping);
            inner_str = preformat_rep_bodies(&inner_str, rustfmt_path, edition, config_path);
            inner_str = normalize_body_indent(&inner_str);
            all_replaced_bodies_str.push(inner_str);
            all_mappings.push(mapping);
        }
    }
    let shadow_code = build_shadow_file_from_strings(&all_replaced_bodies_str);
    let formatted_shadow = run_rustfmt(&shadow_code, rustfmt_path, edition, config_path)?;
    let result = apply_formatting(source, &macro_defs, &formatted_shadow, &all_mappings);
    final_format_pass(&result, rustfmt_path, edition, config_path)
}