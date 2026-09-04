pub mod formatter;
pub mod mapper;
pub mod parser;
pub mod replacer;
pub mod shadow;
pub mod types;

#[cfg(test)]
mod tests;

use crate::formatter::{run_rustfmt, run_rustfmt_no_macro};
use crate::mapper::{apply_formatting, format_definition_without_brace_bodies};
use crate::parser::parse_macro_defs;
use crate::replacer::replace_macro_syntax_text;
use crate::shadow::build_shadow_file_from_strings;
use crate::types::{FormatOptions, FormatResult, MacroOutcome, MacroStatus, Mapping};
use ra_ap_rustc_lexer::{tokenize, FrontmatterAllowed, TokenKind};
use std::collections::HashSet;
use std::ops::Range;

fn try_format_as_mod(
    inner: &str,
    id: usize,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> Option<String> {
    // A `mod` wrapper only parses when the fragment is a sequence of items,
    // so for the common statement-body case rustfmt is guaranteed to reject
    // it and the caller falls through to `try_format_as_fn` anyway. The
    // first token settles it without paying a process spawn.
    //
    // ponytail: conservative, not exact -- `syn::parse_file` would be exact,
    // but it is the only syn call in the crate and linking its parser took
    // the binary from 1.1 to 3.2 MB, which every user downloads. Being wrong
    // here only costs the spawn we would have made anyway.
    let inner_tokens = layout_tokens(inner);
    let first = next_layout_token(&inner_tokens, 0)?;
    if !is_item_start(inner, &inner_tokens[first]) {
        return None;
    }
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

fn find_rep_markers(body_str: &str, repetition_prefix: &str) -> Vec<RepMarker> {
    let bytes = body_str.as_bytes();
    let mut markers = Vec::new();
    let mut i = 0;
    let mut rep_id = 0;
    while i < bytes.len() {
        if body_str[i..].starts_with(repetition_prefix) {
            let kind_start = i + repetition_prefix.len();
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
            i += body_str[i..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        }
    }
    markers
}
fn preformat_rep_bodies(
    body_str: &str,
    repetition_prefix: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> String {
    let markers = find_rep_markers(body_str, repetition_prefix);
    if markers.is_empty() {
        return body_str.to_string();
    }
    let mut result = body_str.to_string();
    for m in markers.into_iter().rev() {
        let inner = result[m.inner_start..m.inner_end].to_string();
        let formatted = try_format_as_mod(&inner, m.rep_id, rustfmt_path, edition, config_path)
            .or_else(|| try_format_as_fn(&inner, m.rep_id, rustfmt_path, edition, config_path));
        if let Some(fmt) = formatted.filter(|fmt| ensure_tokens_preserved(&inner, fmt).is_ok()) {
            if fmt.contains('\n') || fmt.trim_end().ends_with(';') {
                result.replace_range(
                    m.inner_start..m.inner_end,
                    &format!("\n{}\n", fmt.trim_matches('\n')),
                );
            } else {
                result.replace_range(m.inner_start..m.inner_end, &fmt);
            }
        }
    }
    result
}
fn is_rep_opener(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("$(") && !s[2..].contains(')')
}

fn is_rep_closer(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with(')') {
        return false;
    }
    let after_close = &s[1..];
    if after_close.is_empty() {
        return false;
    }
    let after_sep = if after_close.starts_with(',') || after_close.starts_with(';') {
        &after_close[1..]
    } else {
        after_close
    };
    matches!(after_sep, "+" | "*" | "?")
}

fn normalize_body_indent(body: &str) -> String {
    const BASE_INDENT: usize = 4;
    const UNIT: usize = 4;

    let lines: Vec<&str> = body.lines().collect();
    if lines.len() <= 1 {
        return body.to_string();
    }
    let mut result = Vec::with_capacity(lines.len());
    let mut depth = 0usize;
    let mut in_where = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            result.push(String::new());
            continue;
        }

        let is_open_brace = trimmed.ends_with('{');
        let is_close_brace = trimmed.starts_with('}');
        let is_open_rep = is_rep_opener(trimmed);
        let is_close_rep = is_rep_closer(trimmed);
        let is_where = trimmed == "where";

        // Closing lines indent at the DECREASED depth
        let effective_depth = if is_close_brace || is_close_rep {
            depth.saturating_sub(1)
        } else {
            depth
        };

        // Where clause content gets +4 indent
        let where_extra = if in_where && !is_where && !is_open_brace && !is_close_brace {
            UNIT
        } else {
            0
        };

        let indent = BASE_INDENT + effective_depth * UNIT + where_extra;
        result.push(format!("{:indent$}{}", "", trimmed, indent = indent));

        // Update state after computing indent
        if is_open_brace || is_open_rep {
            depth += 1;
        }
        if is_close_brace || is_close_rep {
            depth = depth.saturating_sub(1);
        }
        if is_where {
            in_where = true;
        }
        if is_open_brace {
            in_where = false;
        }
    }

    result.join("\n")
}

fn final_format_pass(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    options: FormatOptions,
) -> anyhow::Result<String> {
    let definitions = parse_macro_defs(source)?;
    let skipped = definitions
        .iter()
        .filter(|definition| !supports_deep_format(source, definition))
        .collect::<Vec<_>>();
    let mut masked = source.to_string();
    let marker = unique_skip_marker(source);
    for (index, definition) in skipped.iter().enumerate().rev() {
        masked.replace_range(definition.span.clone(), &format!("/*{marker}{index}*/"));
    }
    let mut formatted = run_rustfmt_no_macro(&masked, rustfmt_path, edition, config_path)?;
    for (index, definition) in skipped.iter().enumerate() {
        let placeholder = format!("/*{marker}{index}*/");
        let position = formatted
            .find(&placeholder)
            .ok_or_else(|| anyhow::anyhow!("rustfmt removed a skipped macro marker"))?;
        formatted.replace_range(
            position..position + placeholder.len(),
            &source[definition.span.clone()],
        );
    }
    let invocations = format_macro_invocations(&formatted, rustfmt_path, edition, config_path)?;
    Ok(normalize_layout_gaps(&invocations, options))
}

fn unique_skip_marker(source: &str) -> String {
    (0..)
        .map(|index| format!("__m_skip_{index}_"))
        .find(|candidate| !source.contains(candidate))
        .expect("infinite marker namespace")
}

#[derive(Clone)]
struct LayoutToken {
    kind: TokenKind,
    span: Range<usize>,
}

fn layout_tokens(source: &str) -> Vec<LayoutToken> {
    let mut offset = 0usize;
    tokenize(source, FrontmatterAllowed::Yes)
        .map(|token| {
            let start = offset;
            offset += token.len as usize;
            LayoutToken {
                kind: token.kind,
                span: start..offset,
            }
        })
        .collect()
}

fn is_layout_trivia(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Whitespace)
}

fn next_layout_token(tokens: &[LayoutToken], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|&index| !is_layout_trivia(tokens[index].kind))
}

fn matching_layout_delimiter(tokens: &[LayoutToken], open: usize) -> Option<usize> {
    let expected = match tokens[open].kind {
        TokenKind::OpenParen => TokenKind::CloseParen,
        TokenKind::OpenBrace => TokenKind::CloseBrace,
        TokenKind::OpenBracket => TokenKind::CloseBracket,
        _ => return None,
    };
    let mut stack = vec![expected];
    for (index, token) in tokens.iter().enumerate().skip(open + 1) {
        let close = match token.kind {
            TokenKind::OpenParen => Some(TokenKind::CloseParen),
            TokenKind::OpenBrace => Some(TokenKind::CloseBrace),
            TokenKind::OpenBracket => Some(TokenKind::CloseBracket),
            _ => None,
        };
        if let Some(close) = close {
            stack.push(close);
        } else if matches!(
            token.kind,
            TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket
        ) {
            if stack.pop()? != token.kind {
                return None;
            }
            if stack.is_empty() {
                return Some(index);
            }
        }
    }
    None
}

struct Invocation {
    name_start: usize,
    bang_end: usize,
    open_start: usize,
    inner: Range<usize>,
    end: usize,
    open: char,
}

fn macro_invocations(source: &str, macro_names: &HashSet<String>) -> Vec<Invocation> {
    let tokens = layout_tokens(source);
    let mut invocations: Vec<Invocation> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Ident | TokenKind::RawIdent) {
            continue;
        }
        let name = &source[token.span.clone()];
        if !macro_names.contains(name) && is_builtin_macro(name) {
            continue;
        }
        let Some(bang) = next_layout_token(&tokens, index + 1) else {
            continue;
        };
        if tokens[bang].kind != TokenKind::Bang {
            continue;
        }
        let Some(open) = next_layout_token(&tokens, bang + 1) else {
            continue;
        };
        let Some(close) = matching_layout_delimiter(&tokens, open) else {
            continue;
        };
        let candidate = Invocation {
            name_start: token.span.start,
            bang_end: tokens[bang].span.end,
            open_start: tokens[open].span.start,
            inner: tokens[open].span.end..tokens[close].span.start,
            end: tokens[close].span.end,
            open: match tokens[open].kind {
                TokenKind::OpenBrace => '{',
                TokenKind::OpenBracket => '[',
                _ => '(',
            },
        };
        if invocations
            .last()
            .is_some_and(|parent| candidate.end <= parent.end)
        {
            continue;
        }
        invocations.push(candidate);
    }
    invocations
}

fn is_builtin_macro(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "cfg"
            | "column"
            | "compile_error"
            | "concat"
            | "dbg"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "env"
            | "eprint"
            | "eprintln"
            | "file"
            | "format"
            | "format_args"
            | "include"
            | "include_bytes"
            | "include_str"
            | "line"
            | "matches"
            | "module_path"
            | "option_env"
            | "panic"
            | "print"
            | "println"
            | "stringify"
            | "thread_local"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "vec"
            | "write"
            | "writeln"
    )
}

fn format_macro_invocations(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let mut result = source.to_string();
    let macro_names = parse_macro_defs(source)?
        .into_iter()
        .map(|definition| definition.name)
        .collect::<HashSet<_>>();
    for invocation in macro_invocations(source, &macro_names).into_iter().rev() {
        let inner = &source[invocation.inner.clone()];
        let trimmed = inner.trim();
        if trimmed.is_empty()
            || contains_rust_comment(inner)
            || (invocation.open == '(' && trimmed.starts_with('{') && trimmed.ends_with('}'))
        {
            continue;
        }
        let line_start = source[..invocation.name_start]
            .rfind('\n')
            .map_or(0, |position| position + 1);
        let base_indent = source[line_start..invocation.name_start]
            .chars()
            .take_while(|character| character.is_whitespace())
            .count();
        let prefix_width = source[line_start..invocation.inner.start].chars().count();
        let formatted = format_invocation_inner(
            inner,
            invocation.open,
            base_indent,
            prefix_width,
            rustfmt_path,
            edition,
            config_path,
        );
        result.replace_range(invocation.inner, &formatted);
        result.replace_range(
            invocation.bang_end..invocation.open_start,
            if invocation.open == '{' { " " } else { "" },
        );
    }
    ensure_tokens_preserved(source, &result)?;
    Ok(result)
}

fn contains_rust_comment(source: &str) -> bool {
    tokenize(source, FrontmatterAllowed::No).any(|token| {
        matches!(
            token.kind,
            TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        )
    })
}

fn format_invocation_inner(
    inner: &str,
    open: char,
    base_indent: usize,
    prefix_width: usize,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> String {
    let trimmed = inner.trim();
    let looks_like_rust = trimmed.starts_with("#")
        || trimmed.starts_with("pub ")
        || trimmed.starts_with("fn ")
        || trimmed.starts_with("struct ")
        || trimmed.starts_with("enum ")
        || trimmed.starts_with("impl ")
        || trimmed.starts_with("let ")
        || trimmed.starts_with('{');
    if looks_like_rust {
        let formatted = try_format_as_mod(trimmed, 0, rustfmt_path, edition, config_path)
            .or_else(|| try_format_as_fn(trimmed, 0, rustfmt_path, edition, config_path));
        if let Some(formatted) =
            formatted.filter(|text| ensure_tokens_preserved(trimmed, text).is_ok())
        {
            return indent_invocation_body(&formatted, base_indent);
        }
    }

    let canonical = mapper::canonical_token_spacing(trimmed);
    let expanded_item = mapper::expand_inline_structs(&canonical);
    if expanded_item.contains('\n') {
        return indent_invocation_body(&expanded_item, base_indent);
    }
    if let Some(block) = format_named_brace_list(&canonical, base_indent) {
        return if open == '{' {
            indent_invocation_body(&block, base_indent)
        } else {
            block
        };
    }

    // Leave one column for a possible trailing semicolon after the invocation.
    let width_limit = 79;
    if !canonical.contains('\n') && prefix_width + canonical.len() + 1 <= width_limit {
        return if open == '{' {
            format!(" {canonical} ")
        } else {
            canonical
        };
    }
    if let Some(list) = format_dsl_comma_list(&canonical, base_indent) {
        return list;
    }

    let lines = inner
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| mapper::canonical_token_spacing(line.trim()))
        .collect::<Vec<_>>();
    if lines.len() <= 1 {
        canonical
    } else {
        indent_invocation_body(&lines.join("\n"), base_indent)
    }
}

fn format_dsl_comma_list(source: &str, base_indent: usize) -> Option<String> {
    let tokens = parser::significant_tokens(source).ok()?;
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut items = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," if depth == 0 => {
                if start < index {
                    let text = mapper::canonical_token_spacing(
                        &source[tokens[start].span.start..token.span.start],
                    );
                    items.push(text);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let trailing_comma = tokens.last().is_some_and(|token| token.text == ",");
    if start < tokens.len() {
        let text = mapper::canonical_token_spacing(
            &source[tokens[start].span.start..tokens.last()?.span.end],
        );
        items.push(text);
    }
    if items.len() < 2 {
        return None;
    }
    // Pack items greedily, the same way rustfmt wraps a builtin call like
    // `vec![...]`, instead of forcing one item per line.
    const STYLE_WIDTH: usize = 100;
    let item_indent = base_indent + 4;
    let line_width_limit = STYLE_WIDTH.saturating_sub(item_indent);
    let mut output = String::new();
    let item_count = items.len();
    let mut current_len = 0usize;
    let mut line_has_item = false;
    for (index, item) in items.into_iter().enumerate() {
        let is_last = index + 1 == item_count;
        let has_comma = !is_last || trailing_comma;
        let piece_len = item.chars().count() + usize::from(has_comma);
        if !line_has_item {
            output.push('\n');
            output.push_str(&" ".repeat(item_indent));
            current_len = 0;
        } else if current_len + 1 + piece_len > line_width_limit {
            output.push('\n');
            output.push_str(&" ".repeat(item_indent));
            current_len = 0;
        } else {
            output.push(' ');
            current_len += 1;
        }
        output.push_str(&item);
        current_len += item.chars().count();
        if has_comma {
            output.push(',');
            current_len += 1;
        }
        line_has_item = true;
    }
    output.push('\n');
    output.push_str(&" ".repeat(base_indent));
    Some(output)
}

fn indent_invocation_body(body: &str, base_indent: usize) -> String {
    let lines = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let minimum = lines
        .iter()
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut result = String::new();
    for line in lines {
        result.push('\n');
        result.push_str(&" ".repeat(base_indent + 4));
        let indent = line.len() - line.trim_start().len();
        result.push_str(&" ".repeat(indent.saturating_sub(minimum)));
        result.push_str(line.trim_start());
    }
    result.push('\n');
    result.push_str(&" ".repeat(base_indent));
    result
}

fn format_named_brace_list(source: &str, base_indent: usize) -> Option<String> {
    let tokens = parser::significant_tokens(source).ok()?;
    if tokens.len() < 4
        || !matches!(tokens[0].kind.as_str(), "Ident" | "RawIdent")
        || tokens[1].text != "{"
        || tokens.last()?.text != "}"
    {
        return None;
    }
    let mut depth = 0usize;
    let mut item_start = 2usize;
    let mut items = Vec::new();
    for index in 2..tokens.len() - 1 {
        match tokens[index].text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," if depth == 0 => {
                if item_start < index {
                    items.push(tokens[item_start].span.start..tokens[index].span.end);
                }
                item_start = index + 1;
            }
            _ => {}
        }
    }
    if items.len() < 2 {
        return None;
    }
    let mut result = format!("{} {{", tokens[0].text);
    for item in items {
        result.push('\n');
        result.push_str(&" ".repeat(base_indent + 4));
        result.push_str(&mapper::canonical_token_spacing(&source[item]));
    }
    result.push('\n');
    result.push_str(&" ".repeat(base_indent));
    result.push('}');
    Some(result)
}

fn normalize_layout_gaps(source: &str, options: FormatOptions) -> String {
    let tokens = layout_tokens(source);
    let macro_names = parse_macro_defs(source)
        .unwrap_or_default()
        .into_iter()
        .map(|definition| definition.name)
        .collect::<HashSet<_>>();
    let significant = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| !is_layout_trivia(token.kind))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut depth = 0usize;
    let mut depth_after = vec![0usize; tokens.len()];
    let mut container_stack = Vec::new();
    let mut item_container_after = vec![false; tokens.len()];
    for &index in &significant {
        match tokens[index].kind {
            TokenKind::OpenParen | TokenKind::OpenBracket => {
                depth += 1;
                container_stack.push(false);
            }
            TokenKind::OpenBrace => {
                depth += 1;
                container_stack.push(brace_opens_item_container(
                    source,
                    &tokens,
                    &significant,
                    index,
                ));
            }
            TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket => {
                depth = depth.saturating_sub(1);
                container_stack.pop();
            }
            _ => {}
        }
        depth_after[index] = depth;
        item_container_after[index] = container_stack.last().copied().unwrap_or(false);
    }

    let mut replacements = Vec::new();
    for pair in significant.windows(2) {
        let previous_index = pair[0];
        let next_index = pair[1];
        let previous = &tokens[previous_index];
        let next = &tokens[next_index];
        let gap = &source[previous.span.end..next.span.start];
        if !gap.contains('\n') {
            continue;
        }
        let indent = gap.rsplit_once('\n').map_or("", |(_, indent)| indent);
        let blank_lines = gap.bytes().filter(|byte| *byte == b'\n').count();

        let top_level_boundary = depth_after[previous_index] == 0
            && (previous.kind == TokenKind::CloseBrace
                || previous.kind == TokenKind::Semi
                    && top_level_semi_needs_blank(
                        source,
                        &tokens,
                        &significant,
                        &depth_after,
                        previous_index,
                        next_index,
                    ));
        let nested_item_boundary = previous.kind == TokenKind::CloseBrace
            && item_container_after[previous_index]
            && is_item_start(source, next);
        // Every rule below only ever *removes* vertical space, and rustfmt
        // keeps all of it, so the whole family lives behind one switch: with
        // it off this crate leaves the author's blank lines exactly where
        // rustfmt would. The rules that insert blank lines are unaffected.
        let compact = options.compact_blank_lines;
        let nested_gap = compact && depth_after[previous_index] > 0 && blank_lines > 1;
        let comma_gap = compact && previous.kind == TokenKind::Comma && blank_lines > 1;
        let previous_macro = preceding_macro_name(source, &tokens, &significant, previous_index);
        let repeated_macro_gap = compact
            && previous.kind == TokenKind::Semi
            && blank_lines > 1
            && previous_macro == following_macro_name(source, &tokens, next_index)
            && previous_macro.is_some_and(|name| macro_names.contains(name));
        let attribute_gap = compact
            && blank_lines > 1
            && previous.kind == TokenKind::CloseBracket
            && is_attribute_close(&tokens, &significant, previous_index);
        // ponytail: `use` is deliberately excluded. Collapsing a blank line
        // between two `use` items merges two import groups, and rustfmt then
        // sorts across the seam, which the token-preservation oracle reads as
        // reordered code and aborts the whole file on. The same hazard exists
        // for deliberately unsorted `mod` declarations; that case degrades to
        // a plain-rustfmt fallback rather than bad output, so it is left
        // alone. Compare the two names here if it ever shows up in practice.
        let compact_module_gap = compact
            && blank_lines > 1
            && depth_after[previous_index] == 0
            && previous.kind == TokenKind::Semi
            && preceding_item_is_module(
                source,
                &tokens,
                &significant,
                &depth_after,
                previous_index,
            )
            && following_item_keyword(source, &tokens, &significant, next_index) == Some("mod");

        let desired = if top_level_boundary || nested_item_boundary {
            format!("\n\n{indent}")
        } else if nested_gap
            || comma_gap
            || repeated_macro_gap
            || attribute_gap
            || compact_module_gap
        {
            format!("\n{indent}")
        } else {
            continue;
        };
        if gap != desired {
            replacements.push((previous.span.end..next.span.start, desired));
        }
    }

    let mut result = source.to_string();
    for (span, replacement) in replacements.into_iter().rev() {
        result.replace_range(span, &replacement);
    }
    result
}

fn preceding_item_is_module(
    source: &str,
    tokens: &[LayoutToken],
    significant: &[usize],
    depth_after: &[usize],
    semi: usize,
) -> bool {
    let Some(position) = significant.iter().position(|index| *index == semi) else {
        return false;
    };
    for &index in significant[..position].iter().rev() {
        if depth_after[index] != 0 {
            continue;
        }
        if matches!(tokens[index].kind, TokenKind::Semi | TokenKind::CloseBrace) {
            break;
        }
        if &source[tokens[index].span.clone()] == "mod" {
            return true;
        }
    }
    false
}

fn brace_opens_item_container(
    source: &str,
    tokens: &[LayoutToken],
    significant: &[usize],
    open: usize,
) -> bool {
    let Some(position) = significant.iter().position(|index| *index == open) else {
        return false;
    };
    for &index in significant[..position].iter().rev() {
        if matches!(
            tokens[index].kind,
            TokenKind::OpenBrace | TokenKind::CloseBrace | TokenKind::Semi
        ) {
            break;
        }
        match &source[tokens[index].span.clone()] {
            "impl" | "trait" | "mod" => return true,
            "fn" | "struct" | "enum" | "union" | "macro_rules" => return false,
            _ => {}
        }
    }
    false
}

fn is_item_start(source: &str, token: &LayoutToken) -> bool {
    if matches!(
        token.kind,
        TokenKind::LineComment { .. } | TokenKind::BlockComment { .. } | TokenKind::Pound
    ) {
        return true;
    }
    matches!(
        &source[token.span.clone()],
        "pub"
            | "fn"
            | "struct"
            | "enum"
            | "union"
            | "impl"
            | "trait"
            | "type"
            | "const"
            | "static"
            | "mod"
            | "use"
            | "extern"
            | "unsafe"
            | "async"
            | "macro_rules"
    )
}

fn top_level_semi_needs_blank(
    source: &str,
    tokens: &[LayoutToken],
    significant: &[usize],
    depth_after: &[usize],
    semi: usize,
    next: usize,
) -> bool {
    let Some(position) = significant.iter().position(|index| *index == semi) else {
        return false;
    };
    let mut has_macro_bang = false;
    for &index in significant[..position].iter().rev() {
        if depth_after[index] != 0 {
            continue;
        }
        if matches!(tokens[index].kind, TokenKind::Semi | TokenKind::CloseBrace) {
            break;
        }
        if tokens[index].kind == TokenKind::Bang {
            has_macro_bang = true;
        }
        match &source[tokens[index].span.clone()] {
            "mod" | "use" => {
                return !matches!(
                    following_item_keyword(source, tokens, significant, next),
                    Some("mod" | "use")
                )
            }
            "struct" | "type" | "const" | "static" => return true,
            _ => {}
        }
    }
    has_macro_bang
}

/// The item keyword that starts the item at `next`, skipping the modifiers
/// and attributes in front of it.
///
/// Two call sites need different sets of it, which is the whole reason this
/// reports the keyword instead of answering a yes/no question: collapsing a
/// blank line is only ever right between `mod` declarations, while omitting
/// one is right between any two of `mod` and `use`. Folding both into one
/// mod-only predicate put a blank line between every pair of consecutive
/// `use` statements.
fn following_item_keyword<'a>(
    source: &'a str,
    tokens: &[LayoutToken],
    significant: &[usize],
    next: usize,
) -> Option<&'a str> {
    let Some(position) = significant.iter().position(|index| *index == next) else {
        return None;
    };
    significant[position..]
        .iter()
        .take(8)
        .map(|index| &source[tokens[*index].span.clone()])
        .find(|text| {
            !matches!(
                *text,
                "pub" | "unsafe" | "async" | "extern" | "crate" | "#" | "[" | "]"
            )
        })
}

fn is_attribute_close(tokens: &[LayoutToken], significant: &[usize], close: usize) -> bool {
    let Some(mut position) = significant.iter().position(|index| *index == close) else {
        return false;
    };
    let mut depth = 1usize;
    while let Some(previous) = position.checked_sub(1) {
        position = previous;
        let index = significant[position];
        match tokens[index].kind {
            TokenKind::CloseBracket => depth += 1,
            TokenKind::OpenBracket => {
                depth -= 1;
                if depth == 0 {
                    let Some(hash_position) = position.checked_sub(1) else {
                        return false;
                    };
                    return tokens[significant[hash_position]].kind == TokenKind::Pound;
                }
            }
            _ => {}
        }
    }
    false
}

fn preceding_macro_name<'a>(
    source: &'a str,
    tokens: &[LayoutToken],
    significant: &[usize],
    before: usize,
) -> Option<&'a str> {
    let position = significant.iter().position(|index| *index == before)?;
    for &index in significant[..position].iter().rev().take(32) {
        if tokens[index].kind == TokenKind::Bang {
            let ident_position = significant
                .iter()
                .position(|candidate| *candidate == index)?;
            let ident = *significant.get(ident_position.checked_sub(1)?)?;
            if matches!(tokens[ident].kind, TokenKind::Ident | TokenKind::RawIdent) {
                return Some(&source[tokens[ident].span.clone()]);
            }
        }
        if matches!(tokens[index].kind, TokenKind::Semi | TokenKind::OpenBrace) {
            break;
        }
    }
    None
}

fn following_macro_name<'a>(
    source: &'a str,
    tokens: &[LayoutToken],
    start: usize,
) -> Option<&'a str> {
    if !matches!(tokens[start].kind, TokenKind::Ident | TokenKind::RawIdent) {
        return None;
    }
    let bang = next_layout_token(tokens, start + 1)?;
    (tokens[bang].kind == TokenKind::Bang).then(|| &source[tokens[start].span.clone()])
}

struct OnceResult {
    text: String,
    skipped_reasons: Vec<Option<String>>,
}

/// Decide whether a batched shadow-format result is safe to apply. Returns
/// `Some(candidate)` when the batch produced output that exactly preserves
/// the original's significant tokens; `None` means the caller must fall
/// back to formatting definitions one at a time.
fn accepted_batch_result(original: &str, batch_result: anyhow::Result<String>) -> Option<String> {
    match batch_result {
        Ok(candidate) if ensure_tokens_preserved(original, &candidate).is_ok() => Some(candidate),
        _ => None,
    }
}

/// Extract, normalize, and pre-format one macro arm's shadow-file body
/// fragment. Shared by `format_definition_once` (single-definition path)
/// and `format_definitions_batch` (multi-definition path) so the two never
/// drift out of sync with each other.
fn build_arm_shadow_fragment(
    source: &str,
    arm: &crate::types::MacroArm,
    marker_prefix: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> (String, Mapping) {
    let arm_body_text = &source[arm.body_span.clone()];
    let body_text = arm_body_text.trim();
    let inner = if body_text.starts_with("{{") && body_text.ends_with("}}") {
        &body_text[2..body_text.len() - 2]
    } else {
        &body_text[1..body_text.len() - 1]
    };
    let trimmed = inner.strip_prefix('\n').unwrap_or(inner);
    let inner_text = trimmed.strip_suffix('\n').unwrap_or(trimmed);
    let mut mapping = Mapping::with_prefix(marker_prefix.to_string());
    let mut inner_str = normalize_body_indent(inner_text);
    inner_str = replace_macro_syntax_text(&inner_str, &mut mapping);
    inner_str = preformat_rep_bodies(
        &inner_str,
        &format!("{marker_prefix}rep_"),
        rustfmt_path,
        edition,
        config_path,
    );
    (inner_str, mapping)
}

/// Format a single macro definition. This is the N=1 special case of
/// `format_definitions_batch` — kept as a separate named function because
/// the fallback path in `format_source_once` calls it per-definition when a
/// batch fails the token-preservation check, but its body is exactly
/// `format_definitions_batch` with a one-element slice so the fallback is
/// structurally guaranteed to reproduce the same formatting as the batch
/// path, rather than relying on two hand-maintained copies staying in sync.
fn format_definition_once(
    source: &str,
    definition: &crate::types::MacroDef,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    if !supports_deep_format(source, definition) {
        return Ok(format_definition_without_brace_bodies(source, definition));
    }
    format_definitions_batch(source, &[definition], rustfmt_path, edition, config_path)
}

fn supports_deep_format(source: &str, definition: &crate::types::MacroDef) -> bool {
    definition.arms.iter().all(|arm| {
        let body = source[arm.body_span.clone()].trim();
        body.starts_with('{') && body.ends_with('}')
    })
}

/// Format every arm of every given definition in one combined shadow file
/// and one `rustfmt` call, instead of one call per definition. Definitions
/// must already be filtered to `supports_deep_format` and given in
/// ascending source-position order (the natural order `parse_macro_defs`
/// returns).
fn format_definitions_batch(
    source: &str,
    definitions: &[&crate::types::MacroDef],
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let marker_prefix = unique_prefix(source);
    let mut all_replaced_bodies_str: Vec<String> = Vec::new();
    let mut all_mappings: Vec<Mapping> = Vec::new();
    for definition in definitions {
        for arm in &definition.arms {
            let (inner_str, mapping) = build_arm_shadow_fragment(
                source,
                arm,
                &marker_prefix,
                rustfmt_path,
                edition,
                config_path,
            );
            all_replaced_bodies_str.push(inner_str);
            all_mappings.push(mapping);
        }
    }
    let shadow_code = build_shadow_file_from_strings(&all_replaced_bodies_str, &marker_prefix);
    let formatted_shadow = run_rustfmt(&shadow_code, rustfmt_path, edition, config_path)?;
    let owned_definitions: Vec<crate::types::MacroDef> = definitions
        .iter()
        .map(|definition| (*definition).clone())
        .collect();
    Ok(apply_formatting(
        source,
        &owned_definitions,
        &formatted_shadow,
        &all_mappings,
    ))
}

/// Try to format `batchable` definitions with one combined `batch` call; if
/// the result is rejected by `accepted_batch_result` (the batch call
/// errored, or its output fails the token-preservation check), fall back to
/// formatting each definition individually via `format_definition_once`, so
/// a single problematic definition among many healthy ones is still
/// isolated and reported `SKIPPED` individually instead of the whole batch
/// failing.
///
/// `batch` is injected rather than calling `format_definitions_batch`
/// directly so this can be unit-tested without a real `rustfmt` binary:
/// production code (`format_source_once`) passes a closure that calls
/// `format_definitions_batch`; tests pass a closure that deliberately
/// errors or returns token-corrupting output, to prove the fallback loop
/// actually runs end-to-end and still formats the healthy definitions in
/// the same call correctly.
fn apply_deep_definitions_batch(
    mut text: String,
    batchable: &[(usize, &crate::types::MacroDef)],
    skipped_reasons: &mut [Option<String>],
    batch: impl FnOnce(&str, &[&crate::types::MacroDef]) -> anyhow::Result<String>,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> String {
    if batchable.is_empty() {
        return text;
    }
    let just_defs: Vec<&crate::types::MacroDef> = batchable
        .iter()
        .map(|(_, definition)| *definition)
        .collect();
    let batch_result = batch(&text, &just_defs);
    match accepted_batch_result(&text, batch_result) {
        Some(candidate) => text = candidate,
        None => {
            // Fall back to the proven one-call-per-definition path so a
            // single problematic definition among many healthy ones is
            // still isolated and reported SKIPPED individually.
            for &(index, definition) in batchable.iter().rev() {
                match format_definition_once(&text, definition, rustfmt_path, edition, config_path)
                {
                    Ok(candidate) => match ensure_tokens_preserved(&text, &candidate) {
                        Ok(()) => text = candidate,
                        Err(error) => {
                            if let Some(slot) = skipped_reasons.get_mut(index) {
                                *slot = Some(format!("lossless check failed: {error}"));
                            }
                        }
                    },
                    Err(error) => {
                        if let Some(slot) = skipped_reasons.get_mut(index) {
                            *slot = Some(format!("shadow formatting failed: {error}"));
                        }
                    }
                }
            }
        }
    }
    text
}

fn format_source_once(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    options: FormatOptions,
) -> anyhow::Result<OnceResult> {
    let definitions = parse_macro_defs(source)?;
    let mut text = source.to_string();
    let mut skipped_reasons = vec![None; definitions.len()];

    // Phase 1: definitions that cannot be deep-formatted get a pure string
    // transform only (no rustfmt call). Reverse order keeps byte offsets of
    // not-yet-processed definitions valid while `text` is being edited.
    let mut phase1_changed = false;
    for (index, definition) in definitions.iter().enumerate().rev() {
        if supports_deep_format(source, definition) {
            continue;
        }
        let candidate = format_definition_without_brace_bodies(&text, definition);
        match ensure_tokens_preserved(&text, &candidate) {
            Ok(()) => {
                text = candidate;
                phase1_changed = true;
            }
            Err(error) => {
                if let Some(slot) = skipped_reasons.get_mut(index) {
                    *slot = Some(format!("lossless check failed: {error}"));
                }
            }
        }
    }

    // Phase 2: deep-formattable definitions. Only re-parse when phase 1
    // actually rewrote something: a re-parse is itself a hard-failure
    // surface, and if nothing changed the original spans are still valid.
    // Phase 1 never adds, removes, or reorders definitions, so index i in
    // `definitions` still matches index i here (checked defensively below).
    let refreshed_definitions: Vec<crate::types::MacroDef> = if phase1_changed {
        parse_macro_defs(&text)?
    } else {
        definitions.clone()
    };
    assert_eq!(refreshed_definitions.len(), definitions.len());

    let deep_definitions: Vec<(usize, &crate::types::MacroDef)> = refreshed_definitions
        .iter()
        .enumerate()
        .filter(|(_, definition)| supports_deep_format(&text, definition))
        .collect();

    if !deep_definitions.is_empty() {
        // apply_formatting's multi-definition batch path assumes strictly
        // ascending, non-overlapping definition spans (each definition's
        // span.start must be >= the previous one's span.end). That
        // normally holds, but parse_macro_defs's attribute/doc-comment
        // heuristic can pull a later definition's span.start backward into
        // an earlier definition's trailing-comment line (e.g. a `} // [x]`
        // closing line immediately followed by another `macro_rules!`),
        // producing a partial overlap that survives the containment
        // filter. Reduce the batch to a strictly non-overlapping, ascending
        // subset and route any excluded (overlapping) definitions through
        // the proven one-call-per-definition path instead, so a rare
        // overlap degrades to today's per-definition behavior for just the
        // affected definitions rather than panicking the whole batch.
        let mut batchable: Vec<(usize, &crate::types::MacroDef)> = Vec::new();
        let mut excluded_indices: Vec<usize> = Vec::new();
        let mut last_end: Option<usize> = None;
        for &(index, definition) in &deep_definitions {
            let overlaps_previous = last_end.is_some_and(|end| definition.span.start < end);
            if overlaps_previous {
                excluded_indices.push(index);
            } else {
                last_end = Some(definition.span.end);
                batchable.push((index, definition));
            }
        }

        text = apply_deep_definitions_batch(
            text,
            &batchable,
            &mut skipped_reasons,
            |batch_text, defs| {
                format_definitions_batch(batch_text, defs, rustfmt_path, edition, config_path)
            },
            rustfmt_path,
            edition,
            config_path,
        );

        if !excluded_indices.is_empty() {
            // The batch step above (if it ran) may have shifted byte
            // offsets ahead of these definitions in the file, so re-parse
            // to get their current spans rather than reusing stale ones
            // captured before that edit.
            let current_definitions = parse_macro_defs(&text)?;
            assert_eq!(current_definitions.len(), refreshed_definitions.len());
            for &index in excluded_indices.iter().rev() {
                let Some(definition) = current_definitions.get(index) else {
                    continue;
                };
                match format_definition_once(&text, definition, rustfmt_path, edition, config_path)
                {
                    Ok(candidate) => match ensure_tokens_preserved(&text, &candidate) {
                        Ok(()) => text = candidate,
                        Err(error) => {
                            if let Some(slot) = skipped_reasons.get_mut(index) {
                                *slot = Some(format!("lossless check failed: {error}"));
                            }
                        }
                    },
                    Err(error) => {
                        if let Some(slot) = skipped_reasons.get_mut(index) {
                            *slot = Some(format!("shadow formatting failed: {error}"));
                        }
                    }
                }
            }
        }
    }

    let formatted = final_format_pass(&text, rustfmt_path, edition, config_path, options)?;
    ensure_tokens_preserved_across_rustfmt_pass(&text, &formatted)?;
    Ok(OnceResult {
        text: formatted,
        skipped_reasons,
    })
}

fn unique_prefix(source: &str) -> String {
    (0..)
        .map(|index| format!("__m{index}_"))
        .find(|candidate| !source.contains(candidate))
        .expect("infinite marker namespace")
}

/// Format `source` with the default options, so the common case and every
/// caller that predates `FormatOptions` need not name a struct it has no
/// opinion about.
pub fn format_source(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    format_source_with_options(
        source,
        rustfmt_path,
        edition,
        config_path,
        FormatOptions::default(),
    )
}

pub fn format_source_with_options(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    options: FormatOptions,
) -> anyhow::Result<String> {
    Ok(
        format_source_with_report_and_options(source, rustfmt_path, edition, config_path, options)?
            .text,
    )
}

/// Remap a byte span computed against `lf_text` (line endings already
/// normalized to `\n`) back to the equivalent span in the original text
/// that had `\r\n` at every one of those newlines. Each `\n` at or before
/// an offset accounts for one extra `\r` byte inserted before it.
fn remap_lf_span_to_crlf(span: std::ops::Range<usize>, lf_text: &str) -> std::ops::Range<usize> {
    let start = span.start
        + lf_text[..span.start]
            .bytes()
            .filter(|&b| b == b'\n')
            .count();
    let end = span.end + lf_text[..span.end].bytes().filter(|&b| b == b'\n').count();
    start..end
}

pub fn format_source_with_report(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<FormatResult> {
    format_source_with_report_and_options(
        source,
        rustfmt_path,
        edition,
        config_path,
        FormatOptions::default(),
    )
}

pub fn format_source_with_report_and_options(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    options: FormatOptions,
) -> anyhow::Result<FormatResult> {
    if !source.contains("\r\n") {
        return format_source_with_report_impl(source, rustfmt_path, edition, config_path, options);
    }
    // rustfmt (and this crate's own shadow-file processing) silently drops
    // `\r` when fed CRLF input, which the safety oracle then correctly
    // reports as a changed significant token (a LineComment's literal text
    // includes its trailing `\r`). Normalize to `\n` for every internal
    // pass, then restore `\r\n` in the final output and remap every
    // reported span back to the original CRLF source's coordinates.
    let normalized_source = source.replace("\r\n", "\n");
    let mut result = format_source_with_report_impl(
        &normalized_source,
        rustfmt_path,
        edition,
        config_path,
        options,
    )?;
    for outcome in &mut result.macros {
        outcome.span = remap_lf_span_to_crlf(outcome.span.clone(), &normalized_source);
    }
    result.text = result.text.replace('\n', "\r\n");
    Ok(result)
}

fn format_source_with_report_impl(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    options: FormatOptions,
) -> anyhow::Result<FormatResult> {
    const MAX_FORMAT_PASSES: usize = 8;

    let definitions = parse_macro_defs(source)?;
    let mut text = source.to_string();
    let mut skipped_reasons = vec![None; definitions.len()];
    let mut converged = false;
    for _ in 0..MAX_FORMAT_PASSES {
        let pass = format_source_once(&text, rustfmt_path, edition, config_path, options)?;
        ensure_tokens_preserved_across_rustfmt_pass(&text, &pass.text)?;
        for (stored, reason) in skipped_reasons.iter_mut().zip(pass.skipped_reasons) {
            if stored.is_none() {
                *stored = reason;
            }
        }
        if pass.text == text {
            converged = true;
            break;
        }
        text = pass.text;
    }
    anyhow::ensure!(
        converged,
        "macro formatting did not converge after {MAX_FORMAT_PASSES} passes"
    );
    run_rustfmt_no_macro(&text, rustfmt_path, edition, config_path)?;
    let formatted_definitions = parse_macro_defs(&text)?;
    let macros = definitions
        .into_iter()
        .enumerate()
        .map(|(index, definition)| MacroOutcome {
            status: if let Some(reason) = &skipped_reasons[index] {
                MacroStatus::Skipped {
                    reason: reason.clone(),
                }
            } else if formatted_definitions.get(index).is_some_and(|formatted| {
                formatted.name == definition.name
                    && text[formatted.span.clone()] == source[definition.span.clone()]
            }) {
                MacroStatus::Unchanged
            } else {
                MacroStatus::Formatted
            },
            name: definition.name,
            span: definition.span,
        })
        .collect();
    Ok(FormatResult { text, macros })
}

/// Assert `after` is a formatting-only rewrite of `before`.
///
/// rustfmt is allowed to *add* `,`, `{` and `}` (a collapsed body regains
/// braces, a wrapped list gains a trailing comma), so those are skipped in
/// `after`. Removals are not tolerated: inside a macro body, dropping the
/// braces of `move || { $body }` changes what the macro expands to, and
/// this oracle is the only thing standing between that and the user's file.
fn ensure_tokens_preserved(before: &str, after: &str) -> anyhow::Result<()> {
    compare_tokens(before, after, false)
}

/// The same check for a whole-file pass of plain rustfmt over real code,
/// where removals are legitimate: a single-expression `match` arm loses its
/// braces once it fits on one line, and a trailing comma disappears when a
/// list collapses. Refusing those aborted formatting for the entire file.
///
/// Only safe because every macro-body rewrite is checked separately by
/// `ensure_tokens_preserved` before it is folded into the file, and this
/// pass runs rustfmt with `format_macro_bodies=false`, so rustfmt does not
/// reach inside a macro body here.
fn ensure_tokens_preserved_across_rustfmt_pass(before: &str, after: &str) -> anyhow::Result<()> {
    compare_tokens(before, after, true)
}

fn compare_tokens(before: &str, after: &str, allow_removals: bool) -> anyhow::Result<()> {
    let before = parser::significant_tokens(before)?;
    let after = parser::significant_tokens(after)?;
    let mut left = 0usize;
    let mut right = 0usize;
    while left < before.len() && right < after.len() {
        if before[left].kind == after[right].kind && before[left].text == after[right].text {
            left += 1;
            right += 1;
        } else if matches!(after[right].text.as_str(), "," | "{" | "}") {
            right += 1;
        } else if allow_removals && matches!(before[left].text.as_str(), "," | "{" | "}") {
            left += 1;
        } else {
            anyhow::bail!(
                "formatter changed significant Rust token {left}: {:?} {:?} -> {:?} {:?}",
                before[left].kind,
                before[left].text,
                after[right].kind,
                after[right].text
            );
        }
    }
    let is_punctuation = |tokens: &[parser::SignificantToken]| {
        tokens
            .iter()
            .all(|token| matches!(token.text.as_str(), "," | "{" | "}"))
    };
    let before_tail_ok = if allow_removals {
        is_punctuation(&before[left..])
    } else {
        left == before.len()
    };
    if !before_tail_ok || !is_punctuation(&after[right..]) {
        anyhow::bail!(
            "formatter removed or changed significant Rust tokens: {} -> {}",
            before.len(),
            after.len()
        );
    }
    Ok(())
}
