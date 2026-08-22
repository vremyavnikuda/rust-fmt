use crate::parser::parse_macro_defs;
use crate::types::{MacroDef, Mapping};
use ra_ap_rustc_lexer::{tokenize, FrontmatterAllowed, TokenKind};

/// Format all macro bodies in the source and return the full formatted source.
///
/// Uses segment-based reconstruction: builds output in linear order,
/// preserving original source for patterns/semicolons/braces and replacing
/// only the inner content of arm bodies.
pub fn apply_formatting(
    original_source: &str,
    macro_defs: &[MacroDef],
    formatted_shadow: &str,
    all_mappings: &[Mapping],
) -> String {
    let sections = split_shadow_into_arms(formatted_shadow);
    let mut result = String::with_capacity(original_source.len());
    let mut source_position = 0usize;
    let mut section_position = 0usize;
    let mut mapping_position = 0usize;

    for definition in macro_defs {
        result.push_str(&original_source[source_position..definition.span.start]);
        let first_arm = &definition.arms[0];
        let header = &original_source[definition.span.start..first_arm.pattern_span.start];
        let keyword = header.find("macro_rules").unwrap_or(0);
        result.push_str(&header[..keyword]);
        let macro_indent = structural_indent(original_source, definition.span.start + keyword);
        let open = header.trim_end().chars().last().unwrap_or('{');
        let close = match open {
            '(' => ')',
            '[' => ']',
            _ => '}',
        };
        let arm_indent = macro_indent + 4;
        result.push_str("macro_rules! ");
        result.push_str(&definition.name);
        result.push(' ');
        result.push(open);
        result.push('\n');

        for (arm_index, arm) in definition.arms.iter().enumerate() {
            let Some(section) = sections.get(section_position + arm_index) else {
                result.push_str(&original_source[arm.pattern_span.start..arm.body_span.end]);
                continue;
            };
            let mapping = &all_mappings[mapping_position + arm_index];
            result.push_str(&" ".repeat(arm_indent));
            push_indented_matcher(
                &mut result,
                arm_indent,
                &format_matcher(&original_source[arm.pattern_span.clone()]),
            );
            result.push_str(" => ");

            let original_body = original_source[arm.body_span.clone()].trim();
            let original_inner = &original_body[1..original_body.len() - 1];
            let formatted_inner = crate::normalize_body_indent(&format_body_spacing(
                &map_arm_section_with_original(section, mapping, original_inner),
            ));
            let double_brace = original_body.starts_with("{{");
            if formatted_inner.trim().is_empty() {
                result.push_str(if double_brace { "{{}}" } else { "{}" });
            } else {
                result.push_str(if double_brace { "{{\n" } else { "{\n" });
                let minimum_indent = formatted_inner
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.len() - line.trim_start().len())
                    .min()
                    .unwrap_or(0);
                for line in formatted_inner
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                {
                    let trimmed = line.trim_start();
                    let relative_indent = line.len() - trimmed.len();
                    result.push_str(
                        &" ".repeat(
                            arm_indent + 4 + relative_indent.saturating_sub(minimum_indent),
                        ),
                    );
                    result.push_str(trimmed);
                    result.push('\n');
                }
                result.push_str(&" ".repeat(arm_indent));
                result.push_str(if double_brace { "}}" } else { "}" });
            }

            let following = definition
                .arms
                .get(arm_index + 1)
                .map_or(definition.span.end, |next| next.pattern_span.start);
            let separator = &original_source[arm.body_span.end..following];
            if separator.contains(';') {
                result.push(';');
            } else if separator.contains(',') {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(macro_indent));
        result.push(close);
        if original_source[definition.span.clone()]
            .trim_end()
            .ends_with(';')
        {
            result.push(';');
        }
        section_position += definition.arms.len();
        mapping_position += definition.arms.len();
        source_position = definition.span.end;
    }

    result.push_str(&original_source[source_position..]);
    result
}

pub(crate) fn format_definition_without_brace_bodies(
    source: &str,
    definition: &MacroDef,
) -> String {
    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..definition.span.start]);
    let first_arm = &definition.arms[0];
    let header = &source[definition.span.start..first_arm.pattern_span.start];
    let keyword = header.find("macro_rules").unwrap_or(0);
    result.push_str(&header[..keyword]);
    let macro_indent = structural_indent(source, definition.span.start + keyword);
    let open = header.trim_end().chars().last().unwrap_or('{');
    let close = match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    };
    let arm_indent = macro_indent + 4;
    result.push_str("macro_rules! ");
    result.push_str(&definition.name);
    result.push(' ');
    result.push(open);
    result.push('\n');

    for (index, arm) in definition.arms.iter().enumerate() {
        result.push_str(&" ".repeat(arm_indent));
        push_indented_matcher(
            &mut result,
            arm_indent,
            &format_matcher(&source[arm.pattern_span.clone()]),
        );
        result.push_str(" => ");
        let body = source[arm.body_span.clone()].trim();
        let body_open = body.chars().next().unwrap_or('{');
        let body_close = body.chars().last().unwrap_or('}');
        let inner = &body[body_open.len_utf8()..body.len() - body_close.len_utf8()];
        result.push(body_open);
        if contains_comment(inner) {
            result.push_str(inner);
        } else {
            result.push_str(&canonical_token_spacing(inner));
        }
        result.push(body_close);

        let following = definition
            .arms
            .get(index + 1)
            .map_or(definition.span.end, |next| next.pattern_span.start);
        let separator = &source[arm.body_span.end..following];
        if separator.contains(';') {
            result.push(';');
        } else if separator.contains(',') {
            result.push(',');
        }
        result.push('\n');
    }

    result.push_str(&" ".repeat(macro_indent));
    result.push(close);
    if source[definition.span.clone()].trim_end().ends_with(';') {
        result.push(';');
    }
    result.push_str(&source[definition.span.end..]);
    result
}

fn push_indented_matcher(output: &mut String, indent: usize, matcher: &str) {
    let mut lines = matcher.lines();
    if let Some(first) = lines.next() {
        output.push_str(first);
    }
    for line in lines {
        output.push('\n');
        output.push_str(&" ".repeat(indent));
        output.push_str(line);
    }
}

fn structural_indent(source: &str, position: usize) -> usize {
    let mut depth = 0usize;
    for token in tokenize(&source[..position], FrontmatterAllowed::Yes) {
        match token.kind {
            TokenKind::OpenBrace => depth += 1,
            TokenKind::CloseBrace => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth * 4
}

fn format_matcher(source: &str) -> String {
    if contains_comment(source) {
        return format_commented_matcher(source);
    }
    let canonical = canonical_token_spacing_with_fragments(source);
    wrap_long_matcher(&canonical)
}

fn wrap_long_matcher(source: &str) -> String {
    const CONTENT_WIDTH: usize = 88;
    if source.chars().count() <= CONTENT_WIDTH {
        return source.to_string();
    }
    let Some(open) = source.chars().next() else {
        return source.to_string();
    };
    let Some(close) = source.chars().last() else {
        return source.to_string();
    };
    if !matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}')) {
        return source.to_string();
    }
    let inner = &source[open.len_utf8()..source.len() - close.len_utf8()];
    let mut words = Vec::new();
    let mut offset = 0usize;
    let mut current = String::new();
    let mut delimiter_depth = 0usize;
    for token in tokenize(inner, FrontmatterAllowed::No) {
        let end = offset + token.len as usize;
        if token.kind == TokenKind::Whitespace {
            if delimiter_depth == 0 && !current.is_empty() {
                words.push(std::mem::take(&mut current));
            } else if !current.is_empty() {
                current.push(' ');
            }
        } else {
            current.push_str(&inner[offset..end]);
            match token.kind {
                TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace => {
                    delimiter_depth += 1
                }
                TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace => {
                    delimiter_depth = delimiter_depth.saturating_sub(1)
                }
                _ => {}
            }
        }
        offset = end;
    }
    if !current.is_empty() {
        words.push(current);
    }
    if words.len() < 2 {
        return source.to_string();
    }

    let mut lines = Vec::new();
    let mut line = String::new();
    for word in words {
        let next_width =
            line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if !line.is_empty() && next_width > CONTENT_WIDTH {
            if line.ends_with(':') {
                if let Some(split) = line.rfind(' ') {
                    let carry = line[split + 1..].to_string();
                    line.truncate(split);
                    lines.push(std::mem::take(&mut line));
                    line = carry;
                } else {
                    lines.push(std::mem::take(&mut line));
                }
            } else {
                lines.push(std::mem::take(&mut line));
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }

    format!(
        "{open}\n{}\n{close}",
        lines
            .into_iter()
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub(crate) fn canonical_token_spacing(source: &str) -> String {
    canonical_token_spacing_impl(source, false)
}

fn canonical_token_spacing_with_fragments(source: &str) -> String {
    canonical_token_spacing_impl(source, true)
}

fn canonical_token_spacing_impl(source: &str, fragment_colons: bool) -> String {
    let tokens = crate::parser::significant_tokens(source).unwrap_or_default();
    let mut output = String::with_capacity(source.len());
    for (index, token) in tokens.iter().enumerate() {
        let current = token.text.as_str();
        let previous = index
            .checked_sub(1)
            .map(|position| tokens[position].text.as_str());
        let before_previous = index
            .checked_sub(2)
            .map(|position| tokens[position].text.as_str());
        let fragment_name = fragment_colons
            && current != "$"
            && previous == Some(":")
            && is_fragment_specifier(&tokens, index);
        let repetition_operator = is_repetition_operator(&tokens, index);
        let repetition_separator = is_repetition_separator(&tokens, index);
        let previous_repetition = index
            .checked_sub(1)
            .is_some_and(|position| is_repetition_operator(&tokens, position));
        let joint_operator = previous.is_some_and(|left| is_joint_operator(left, current));
        let generic_punctuation = is_generic_angle(&tokens, index);
        let previous_unary = index
            .checked_sub(1)
            .is_some_and(|position| is_unary_operator(&tokens, position));
        let at_binding = current == "@"
            && index
                .checked_sub(1)
                .is_some_and(|position| is_fragment_specifier(&tokens, position));
        let previous_at_binding =
            previous == Some("@") && index >= 2 && is_fragment_specifier(&tokens, index - 2);
        let marker_argument = current == "(" && before_previous == Some("@");
        let no_space = index == 0
            || matches!(current, ")" | "]" | "," | ";" | ":" | "." | "!" | "?")
            || current == "@" && !at_binding
            || matches!(previous, Some("(" | "[" | "$" | "#" | "." | "!"))
            || previous == Some("@") && !previous_at_binding
            || (current == "("
                && !marker_argument
                && (matches!(previous, Some(")" | "]" | ">" | "!"))
                    || previous.is_some_and(|_| {
                        tokens[index - 1].kind == "Ident" || tokens[index - 1].kind == "RawIdent"
                    })))
            || fragment_name
            || repetition_operator
            || repetition_separator
            || previous_unary
            || generic_punctuation
            || previous == Some("<") && index >= 2 && is_generic_angle(&tokens, index - 1)
            || joint_operator
            || previous == Some(":") && before_previous == Some(":")
            || previous_repetition && matches!(current, "," | ";" | ")" | "]")
            || (previous == Some("{") && current == "}")
            || (current == "}" && previous == Some("{"));
        if !no_space {
            output.push(' ');
        }
        output.push_str(current);
    }
    output.trim().to_string()
}

fn is_fragment_specifier(tokens: &[crate::parser::SignificantToken], index: usize) -> bool {
    index >= 3
        && tokens[index - 1].text == ":"
        && (tokens[index - 3].text == "$" || index >= 4 && tokens[index - 4].text == "$")
}

fn is_joint_operator(left: &str, right: &str) -> bool {
    matches!(
        (left, right),
        (":", ":")
            | ("-", ">")
            | ("=", ">")
            | ("=", "=")
            | ("!", "=")
            | ("<", "=")
            | (">", "=")
            | ("+", "=")
            | ("-", "=")
            | ("*", "=")
            | ("/", "=")
            | ("%", "=")
            | ("&", "=")
            | ("|", "=")
            | ("^", "=")
            | ("&", "&")
            | ("|", "|")
            | ("<", "<")
            | (">", ">")
            | (".", ".")
            | (".", "=")
    )
}

fn is_repetition_operator(tokens: &[crate::parser::SignificantToken], index: usize) -> bool {
    if !matches!(tokens[index].text.as_str(), "*" | "+" | "?") {
        return false;
    }
    let mut close = match index.checked_sub(1) {
        Some(position) if tokens[position].text == ")" => position,
        Some(position) => match position.checked_sub(1) {
            Some(close) if tokens[close].text == ")" => close,
            _ => return false,
        },
        _ => return false,
    };
    let mut depth = 1usize;
    while let Some(position) = close.checked_sub(1) {
        close = position;
        match tokens[position].text.as_str() {
            ")" => depth += 1,
            "(" => {
                depth -= 1;
                if depth == 0 {
                    return position > 0 && tokens[position - 1].text == "$";
                }
            }
            _ => {}
        }
    }
    false
}

fn is_repetition_separator(tokens: &[crate::parser::SignificantToken], index: usize) -> bool {
    index > 0
        && index + 1 < tokens.len()
        && tokens[index - 1].text == ")"
        && is_repetition_operator(tokens, index + 1)
}

fn is_unary_operator(tokens: &[crate::parser::SignificantToken], index: usize) -> bool {
    let operator = tokens[index].text.as_str();
    if matches!(operator, "!" | "&") {
        return !is_joint_operator(
            operator,
            tokens
                .get(index + 1)
                .map_or("", |token| token.text.as_str()),
        );
    }
    if !matches!(operator, "-" | "*" | "+") || is_repetition_operator(tokens, index) {
        return false;
    }
    let Some(previous) = index
        .checked_sub(1)
        .map(|position| tokens[position].text.as_str())
    else {
        return true;
    };
    matches!(
        previous,
        "(" | "[" | "{" | "," | ";" | ":" | "=" | "=>" | "->" | "return"
    ) || matches!(previous, "+" | "-" | "*" | "/" | "%" | "&&" | "||")
}

fn is_generic_angle(tokens: &[crate::parser::SignificantToken], index: usize) -> bool {
    match tokens[index].text.as_str() {
        "<" => looks_like_generic_open(tokens, index),
        ">" => (0..index).rev().any(|position| {
            tokens[position].text == "<" && looks_like_generic_open(tokens, position)
        }),
        _ => false,
    }
}

fn looks_like_generic_open(tokens: &[crate::parser::SignificantToken], index: usize) -> bool {
    if index == 0 || tokens[index].text != "<" {
        return false;
    }
    let valid_prefix = matches!(tokens[index - 1].text.as_str(), "_" | ")" | "]" | ">")
        || matches!(tokens[index - 1].kind.as_str(), "Ident" | "RawIdent")
        || index >= 2 && tokens[index - 1].text == ":" && tokens[index - 2].text == ":";
    valid_prefix && tokens[index + 1..].iter().any(|token| token.text == ">")
}

fn format_body_spacing(source: &str) -> String {
    if let Some(generated) = format_generated_macro(source) {
        return generated;
    }
    let mut lines = Vec::new();
    for line in source.lines().filter(|line| !line.trim().is_empty()) {
        let indent = line.len() - line.trim_start().len();
        let original = line.trim();
        let mut formatted = if contains_comment(original)
            || matches!(original, ")*" | ")+" | ")?" | "),*" | "),+" | "),?")
        {
            original.to_string()
        } else {
            canonical_token_spacing(original)
        };
        formatted = formatted
            .replace("), *", "),*")
            .replace("), +", "),+")
            .replace("), ?", "),?");
        if formatted.starts_with("$(#") && formatted.contains(")* $") {
            let split = formatted.find(")* $").expect("checked above") + 2;
            lines.push(format!("{}{}", " ".repeat(indent), &formatted[..split]));
            lines.push(format!(
                "{}{}",
                " ".repeat(indent),
                formatted[split..].trim_start()
            ));
        } else if lines.last().is_some_and(|previous: &String| {
            (previous.trim_end().ends_with(" enum")
                || previous.trim_end().ends_with(" struct")
                || previous.trim() == "impl")
                && formatted.starts_with('$')
                || previous.trim() == "$vis" && formatted.starts_with("struct ")
        }) {
            let previous = lines.pop().expect("checked above");
            lines.push(format!("{} {}", previous.trim_end(), formatted));
        } else {
            lines.push(format!("{}{}", " ".repeat(indent), formatted));
        }
    }
    let split = split_partially_expanded_items(&lines.join("\n"));
    let expanded = expand_generated_where_clauses(&expand_inline_structs(&split));
    add_multiline_struct_trailing_commas(&expanded)
}

fn split_partially_expanded_items(source: &str) -> String {
    let mut output = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        let Ok(tokens) = crate::parser::significant_tokens(trimmed) else {
            output.push(line.to_string());
            continue;
        };

        let outer_item = tokens
            .iter()
            .position(|token| matches!(token.text.as_str(), "impl" | "trait" | "mod"));
        let outer_open = outer_item.and_then(|item| {
            tokens[item + 1..]
                .iter()
                .position(|token| token.text == "{")
                .map(|position| item + 1 + position)
        });
        let nested_item = outer_open.and_then(|open| {
            tokens[open + 1..]
                .iter()
                .position(|token| {
                    matches!(
                        token.text.as_str(),
                        "fn" | "impl" | "struct" | "enum" | "type" | "const"
                    )
                })
                .map(|position| open + 1 + position)
        });
        if let Some(nested) = nested_item {
            output.push(format!(
                "{}{}",
                " ".repeat(indent),
                canonical_token_spacing(&trimmed[..tokens[nested].span.start])
            ));
            output.push(format!(
                "{}{}",
                " ".repeat(indent + 4),
                canonical_token_spacing(&trimmed[tokens[nested].span.start..])
            ));
            continue;
        }

        if trimmed.contains("$(")
            && !trimmed.contains('{')
            && tokens.last().is_some_and(|token| token.text == "}")
        {
            let close = tokens.last().expect("checked above").span.start;
            output.push(format!(
                "{}{}",
                " ".repeat(indent),
                canonical_token_spacing(&trimmed[..close])
            ));
            output.push(format!("{}}}", " ".repeat(indent.saturating_sub(4))));
            continue;
        }

        output.push(line.to_string());
    }
    output.join("\n")
}

enum BodyDelimiter {
    Struct { has_field: bool },
    Other,
}

fn add_multiline_struct_trailing_commas(source: &str) -> String {
    let Ok(tokens) = crate::parser::significant_tokens(source) else {
        return source.to_string();
    };
    let mut stack = Vec::new();
    let mut pending_struct_depth = None;
    let mut insertions = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "struct" => pending_struct_depth = Some(stack.len()),
            "(" | "[" => stack.push(BodyDelimiter::Other),
            "{" => {
                if pending_struct_depth == Some(stack.len()) {
                    stack.push(BodyDelimiter::Struct { has_field: false });
                    pending_struct_depth = None;
                } else {
                    stack.push(BodyDelimiter::Other);
                }
            }
            ":" => {
                if let Some(BodyDelimiter::Struct { has_field }) = stack.last_mut() {
                    *has_field = true;
                }
            }
            "," => {
                if let Some(BodyDelimiter::Struct { has_field }) = stack.last_mut() {
                    *has_field = false;
                }
            }
            ")" | "]" => {
                stack.pop();
            }
            "}" => {
                let record = stack.pop();
                if matches!(record, Some(BodyDelimiter::Struct { has_field: true })) {
                    if let Some(previous) = index.checked_sub(1).and_then(|i| tokens.get(i)) {
                        let gap = &source[previous.span.end..token.span.start];
                        if previous.text != "," && gap.contains('\n') {
                            insertions.push(previous.span.end);
                        }
                    }
                }
            }
            ";" if pending_struct_depth == Some(stack.len()) => pending_struct_depth = None,
            _ => {}
        }
    }

    let mut result = source.to_string();
    for position in insertions.into_iter().rev() {
        result.insert(position, ',');
    }
    result
}

fn format_generated_macro(source: &str) -> Option<String> {
    let tokens = crate::parser::significant_tokens(source).ok()?;
    let keyword = tokens
        .iter()
        .position(|token| token.text == "macro_rules")?;
    let bang = keyword + 1;
    if tokens.get(bang)?.text != "!" {
        return None;
    }
    let open = tokens[bang + 1..]
        .iter()
        .position(|token| token.text == "{")?
        + bang
        + 1;
    let close = matching_text_delimiter(&tokens, open)?;
    let pattern_open = open + 1;
    let pattern_close = matching_text_delimiter(&tokens, pattern_open)?;
    let body_open = tokens[pattern_close + 1..close]
        .iter()
        .position(|token| token.text == "{")?
        + pattern_close
        + 1;
    let body_close = matching_text_delimiter(&tokens, body_open)?;
    let name =
        canonical_token_spacing(&source[tokens[bang + 1].span.start..tokens[open].span.start]);
    let pattern = canonical_token_spacing_with_fragments(
        &source[tokens[pattern_open].span.start..tokens[pattern_close].span.end],
    );
    let body =
        canonical_token_spacing(&source[tokens[body_open].span.end..tokens[body_close].span.start]);
    let mut output = format!("macro_rules! {name} {{\n    {pattern} => {{");
    if !body.is_empty() {
        output.push('\n');
        output.push_str("        ");
        output.push_str(&body);
        output.push('\n');
        output.push_str("    ");
    }
    output.push('}');
    if tokens
        .get(body_close + 1)
        .is_some_and(|token| token.text == ";")
    {
        output.push(';');
    }
    output.push_str("\n}");
    Some(output)
}

fn matching_text_delimiter(
    tokens: &[crate::parser::SignificantToken],
    open: usize,
) -> Option<usize> {
    let close = match tokens.get(open)?.text.as_str() {
        "(" => ")",
        "[" => "]",
        "{" => "}",
        _ => return None,
    };
    let mut depth = 1usize;
    for index in open + 1..tokens.len() {
        if tokens[index].text == tokens[open].text {
            depth += 1;
        } else if tokens[index].text == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

pub(crate) fn expand_inline_structs(source: &str) -> String {
    let mut output = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        let Ok(tokens) = crate::parser::significant_tokens(trimmed) else {
            output.push(line.to_string());
            continue;
        };
        let Some(item_token) = tokens.iter().position(|token| {
            matches!(
                token.text.as_str(),
                "struct" | "enum" | "union" | "impl" | "fn" | "trait" | "mod"
            )
        }) else {
            output.push(line.to_string());
            continue;
        };
        if tokens[..item_token]
            .iter()
            .any(|token| matches!(token.text.as_str(), "let" | "=" | "=>" | "return"))
        {
            output.push(line.to_string());
            continue;
        }
        let Some(close_token) = tokens.iter().rposition(|token| token.text == "}") else {
            output.push(line.to_string());
            continue;
        };
        let Some(open_token) = (item_token + 1..close_token).find(|&candidate| {
            tokens[candidate].text == "{"
                && matching_text_delimiter(&tokens, candidate) == Some(close_token)
        }) else {
            output.push(line.to_string());
            continue;
        };
        let open = tokens[open_token].span.start;
        let close = tokens[close_token].span.start;
        let close_end = tokens[close_token].span.end;
        let inner = canonical_token_spacing(&trimmed[open + 1..close]);
        let header = canonical_token_spacing(&trimmed[..open]);
        if header.starts_with("#[") {
            if let Some(attribute_end) = header.find("] ") {
                output.push(format!(
                    "{}{}",
                    " ".repeat(indent),
                    &header[..=attribute_end]
                ));
                output.push(format!(
                    "{}{} {{",
                    " ".repeat(indent),
                    header[attribute_end + 2..].trim_start()
                ));
            } else {
                output.push(format!("{}{} {{", " ".repeat(indent), header));
            }
        } else {
            output.push(format!("{}{} {{", " ".repeat(indent), header));
        }
        if !inner.is_empty() {
            let nested = expand_inline_structs(&inner);
            for nested_line in nested.lines() {
                output.push(format!(
                    "{}{}",
                    " ".repeat(indent + 4),
                    nested_line.trim_start()
                ));
            }
        }
        output.push(format!(
            "{}}}{}",
            " ".repeat(indent),
            canonical_token_spacing(&trimmed[close_end..])
        ));
    }
    output.join("\n")
}

fn expand_generated_where_clauses(source: &str) -> String {
    let mut output = Vec::new();
    let mut awaiting_brace = false;

    for line in source.lines() {
        let trimmed = line.trim();
        let tokens = crate::parser::significant_tokens(trimmed).unwrap_or_default();
        let item = tokens
            .iter()
            .position(|token| matches!(token.text.as_str(), "struct" | "enum" | "union"));
        let where_token = item.and_then(|item| {
            tokens[item + 1..]
                .iter()
                .position(|token| token.text == "where")
                .map(|position| item + 1 + position)
        });

        if let Some(where_token) = where_token {
            let where_start = tokens[where_token].span.start;
            let where_end = tokens[where_token].span.end;
            output.push(canonical_token_spacing(&trimmed[..where_start]));
            output.push("where".to_string());
            let remainder = trimmed[where_end..].trim();
            if let Some(open) = first_brace_span(remainder) {
                push_where_remainder(&mut output, remainder, open);
                awaiting_brace = false;
            } else {
                if !remainder.is_empty() {
                    output.push(canonical_token_spacing(remainder));
                }
                awaiting_brace = true;
            }
            continue;
        }

        if awaiting_brace {
            if let Some(open) = first_brace_span(trimmed) {
                push_where_remainder(&mut output, trimmed, open);
                awaiting_brace = false;
                continue;
            }
        }

        output.push(line.to_string());
    }

    output.join("\n")
}

fn first_brace_span(source: &str) -> Option<std::ops::Range<usize>> {
    crate::parser::significant_tokens(source)
        .ok()?
        .into_iter()
        .rfind(|token| token.text == "{")
        .map(|token| token.span)
}

fn push_where_remainder(output: &mut Vec<String>, remainder: &str, open: std::ops::Range<usize>) {
    let clause = canonical_token_spacing(&remainder[..open.start]);
    if !clause.is_empty() {
        output.push(clause);
    }
    output.push("{".to_string());
    let suffix = remainder[open.end..].trim();
    if !suffix.is_empty() {
        output.push(canonical_token_spacing(suffix));
    }
}

fn format_commented_matcher(source: &str) -> String {
    let trimmed = source.trim();
    let expanded;
    let source = if trimmed.contains("//")
        && matches!(trimmed.as_bytes().first(), Some(b'(' | b'[' | b'{'))
        && matches!(trimmed.as_bytes().last(), Some(b')' | b']' | b'}'))
    {
        expanded = format!(
            "{}\n{}\n{}",
            &trimmed[..1],
            trimmed[1..trimmed.len() - 1].trim_matches('\n'),
            &trimmed[trimmed.len() - 1..]
        );
        expanded.as_str()
    } else {
        source
    };
    let mut output = Vec::new();
    let mut depth = 0usize;
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let closes = line.starts_with([')', ']', '}']);
        if closes {
            depth = depth.saturating_sub(1);
        }
        let formatted = if let Some(comment) = line.find("//") {
            let code = line[..comment].trim();
            if code.is_empty() {
                line.to_string()
            } else {
                format!(
                    "{} {}",
                    canonical_token_spacing_with_fragments(code),
                    &line[comment..]
                )
            }
        } else if contains_comment(line) {
            line.to_string()
        } else {
            canonical_token_spacing_with_fragments(line)
        };
        output.push(format!("{}{}", " ".repeat(depth * 4), formatted));
        if !closes && line.ends_with(['(', '[', '{']) {
            depth += 1;
        }
    }
    output.join("\n")
}

/// Split the formatted shadow file into individual arm body sections.
///
/// Each arm is `macro_rules! __rustfmt_mf_arm_N { () => { BODY }; }`.
/// We extract just the BODY content (inside the `{}` after `=>`).
pub(crate) fn split_shadow_into_arms(shadow_file: &str) -> Vec<String> {
    if let Ok(definitions) = parse_macro_defs(shadow_file) {
        let parsed = definitions
            .into_iter()
            .filter(|definition| definition.name.starts_with("__rustfmt_mf_arm_"))
            .filter_map(|definition| definition.arms.into_iter().next())
            .map(|arm| {
                let body = &shadow_file[arm.body_span];
                let inner = &body[1..body.len() - 1];
                if inner.contains('\n') {
                    inner
                        .strip_prefix('\n')
                        .unwrap_or(inner)
                        .trim_end_matches([' ', '\t'])
                        .strip_suffix('\n')
                        .unwrap_or_else(|| {
                            inner
                                .strip_prefix('\n')
                                .unwrap_or(inner)
                                .trim_end_matches([' ', '\t'])
                        })
                        .to_string()
                } else {
                    inner.trim().to_string()
                }
            })
            .collect::<Vec<_>>();
        if !parsed.is_empty() {
            return parsed;
        }
    }

    let mut sections = Vec::new();
    let mut in_arm = false;
    let mut arm_lines: Vec<&str> = Vec::new();
    let mut rule_indent: Option<usize> = None;
    for line in shadow_file.lines() {
        if detect_arm_opener(line).is_some() {
            if in_arm {
                sections.push(compact_arm_body(&arm_lines));
                arm_lines.clear();
            }
            in_arm = true;
            rule_indent = None;
            // Extract body from single-line arm
            if let Some(body) = extract_arm_body_single(line) {
                let body_str = body.to_string();
                arm_lines.clear();
                sections.push(body_str);
                in_arm = false;
            }
            continue;
        }
        if !in_arm {
            continue;
        }
        // Track indent from the `() => {` line
        if rule_indent.is_none() && line.trim().contains("=> {") {
            rule_indent = Some(line.len() - line.trim_start().len());
            // Single-line arm body: `() => { BODY };` — extract body directly
            let trimmed = line.trim();
            if let Some(arrow_pos) = trimmed.find("=> {") {
                let after_brace = &trimmed[arrow_pos + 4..];
                if let Some(semi_pos) = after_brace.find("};") {
                    let body = after_brace[..semi_pos].to_string();
                    sections.push(body);
                    arm_lines.clear();
                    in_arm = false;
                    rule_indent = None;
                    continue;
                }
            }
            arm_lines.push(line);
            continue;
        }
        // Detect arm closer: `};`, `}};`, or `}` at column 0
        let trimmed = line.trim();
        if in_arm && (trimmed == "};" || trimmed == "}};") {
            let line_indent = line.len() - trimmed.len();
            if rule_indent.map_or(true, |ri| line_indent == ri) {
                sections.push(compact_arm_body(&arm_lines));
                arm_lines.clear();
                in_arm = false;
                rule_indent = None;
                continue;
            }
        }
        arm_lines.push(line);
    }
    if !arm_lines.is_empty() && in_arm {
        sections.push(compact_arm_body(&arm_lines));
    }
    sections
}

/// Given the raw lines of an arm section (from after the `macro_rules!` line
/// to before `};`), extract just the body content from inside `{ ... }`.
fn compact_arm_body(lines: &[&str]) -> String {
    // Find the `() => {` line and track its indent
    let body_start = lines.iter().position(|l| l.trim().contains("=> {"));
    match body_start {
        Some(idx) => {
            let rule_line = lines[idx];
            let rule_indent = rule_line.len() - rule_line.trim_start().len();
            // Scan backwards from end to find last non-empty line
            let end = lines
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .and_then(|last_idx| {
                    let last_line = lines[last_idx];
                    let trimmed = last_line.trim();
                    let line_indent = last_line.len() - trimmed.len();
                    if trimmed == "}" && line_indent == rule_indent {
                        Some(last_idx)
                    } else {
                        None
                    }
                })
                .unwrap_or(lines.len());
            if idx + 1 < end {
                lines[idx + 1..end].join("\n")
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// Extract body content from a single-line arm:
/// "macro_rules! __rustfmt_mf_arm_N { () => { BODY }; }"
fn extract_arm_body_single(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with("macro_rules! __rustfmt_mf_arm_")
        && !trimmed.starts_with("macro_rules ! __rustfmt_mf_arm_")
    {
        return None;
    }
    let body_start = trimmed.find("=> {")?;
    let body_start = body_start + 5;
    let rest = &trimmed[body_start..];
    let body_end = rest.rfind("};")?;
    if body_end > 0 {
        Some(rest[..body_end].trim())
    } else if body_end == 0 {
        Some("")
    } else {
        None
    }
}

/// Remove space between a `$metavar` and `(` in body text.
/// Rustfmt adds a space before `(` inside macro bodies
/// (e.g. `$name ($arg)` → `$name($arg)`).
fn remove_metavar_paren_space(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i;
            i += 1;
            let name_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let name_end = i;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if name_end > name_start && i < bytes.len() && bytes[i] == b'(' {
                result.push_str(&text[start..name_end]);
                result.push('(');
                i += 1;
            } else {
                result.push_str(&text[start..i]);
            }
        } else {
            let ch = text[i..].chars().next().expect("index is a char boundary");
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

/// Collapse a short multi-line body to single line if it fits.
/// Handles cases like `$(#[$attr])*\n$vis fn $name(...)` where rustfmt put
/// a marker on its own line but the total content fits on one line.
/// Avoids collapsing multi-statement bodies (lines ending with `;`),
/// multi-line items (impl, fn, struct, etc.), or comment-attached code.
fn collapse_short_body(text: &str) -> String {
    if !text.contains('\n') {
        return text.to_string();
    }
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() <= 1 {
        return text.to_string();
    }
    for line in &lines {
        let trimmed = line.trim_start();
        if trimmed.ends_with(';')
            || trimmed.ends_with('{')
            || trimmed == "{"
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("macro_rules!")
        {
            return text.to_string();
        }
    }
    if lines.first().is_some_and(|line| line.starts_with("$(")) {
        return text.to_string();
    }
    let single = normalize_body_spacing(&lines.join(" "));
    if single.len() <= 80 {
        single
    } else {
        text.to_string()
    }
}

/// Collapse a simple delimited list (tuple, bracket, block) to single line
/// if it was split by rustfmt unnecessarily (e.g. short tuple `(a, b, c)`).
fn collapse_simple_delimited(text: &str, preserve_trailing_comma: bool) -> String {
    let trimmed = text.trim();
    if !trimmed.contains('\n') {
        return text.to_string();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() {
        return text.to_string();
    }
    let (open, close) = match chars[0] {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        '{' => ('{', '}'),
        _ => return text.to_string(),
    };
    if chars[chars.len() - 1] != close {
        return text.to_string();
    }
    let inner_text = &trimmed[1..trimmed.len() - 1];
    let parts: Vec<&str> = inner_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if parts.is_empty() {
        return format!("{}{}", open, close);
    }
    // Only collapse if every non-empty line ends with ',' (list-like) or is a single-item list
    let all_end_with_comma = parts.iter().all(|p| p.ends_with(','));
    let is_single_item = parts.len() == 1;
    if !all_end_with_comma && !is_single_item {
        return text.to_string();
    }
    let mut joined = parts.join(" ");
    if !preserve_trailing_comma && joined.ends_with(',') {
        joined.pop();
    }
    let single = format!("{}{}{}", open, joined, close);
    if single.len() <= 80 {
        single
    } else {
        text.to_string()
    }
}

/// Detect `macro_rules! __rustfmt_mf_arm_N` or `macro_rules ! __rustfmt_mf_arm_N`
pub(crate) fn detect_arm_opener(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    let after = trimmed
        .strip_prefix("macro_rules! __rustfmt_mf_arm_")
        .or_else(|| trimmed.strip_prefix("macro_rules ! __rustfmt_mf_arm_"))?;
    let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

/// Map a single formatted arm section back to original macro syntax.
///
/// Handles `__mf_rep_*! { ... }` markers inline (not just at line start).
#[cfg(test)]
pub(crate) fn map_arm_section(section: &str, mapping: &Mapping) -> String {
    map_arm_section_with_original(section, mapping, section)
}

fn map_arm_section_with_original(section: &str, mapping: &Mapping, original: &str) -> String {
    let with_reps = replace_rep_markers(section, &format!("{}rep_", mapping.marker_prefix()));
    let restored = restore_placeholders(&with_reps, mapping);
    let spaced = normalize_body_spacing(&restored);
    let spaced = remove_metavar_paren_space(&spaced);
    let spaced = collapse_simple_delimited(&spaced, has_trailing_comma(original));
    collapse_short_body(&spaced)
}

fn has_trailing_comma(text: &str) -> bool {
    let tokens = tokenize(text, FrontmatterAllowed::No)
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
            )
        })
        .map(|token| token.kind)
        .collect::<Vec<_>>();
    tokens.len() >= 2
        && matches!(
            tokens.last(),
            Some(TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace)
        )
        && tokens[tokens.len() - 2] == TokenKind::Comma
}

fn contains_comment(text: &str) -> bool {
    tokenize(text, FrontmatterAllowed::No).any(|token| {
        matches!(
            token.kind,
            TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        )
    })
}

/// Normalize proc_macro2's default spacing inside a single-line
/// macro invocation body.  proc_macro2 adds spaces between every token
/// (e.g. `__m_0 . to_string ()` instead of `__m_0.to_string()`), and
/// because this sits inside `__mf_rep_*!{ … }` rustfmt never touches it.
fn transform_outside_literals_and_comments(
    text: &str,
    transform: impl Fn(&str) -> String,
) -> String {
    let mut result = String::with_capacity(text.len());
    let mut offset = 0usize;
    let mut segment_start = 0usize;
    for token in tokenize(text, FrontmatterAllowed::No) {
        let end = offset + token.len as usize;
        if matches!(
            token.kind,
            TokenKind::Literal { .. }
                | TokenKind::LineComment { .. }
                | TokenKind::BlockComment { .. }
        ) {
            result.push_str(&transform(&text[segment_start..offset]));
            result.push_str(&text[offset..end]);
            segment_start = end;
        }
        offset = end;
    }
    result.push_str(&transform(&text[segment_start..]));
    result
}

fn normalize_inner_spacing(text: &str) -> String {
    transform_outside_literals_and_comments(text, normalize_inner_spacing_raw)
}

fn normalize_inner_spacing_raw(text: &str) -> String {
    let mut result = text.trim().to_string();
    // Collapse space before a lone dot: ` . ` → `.`
    result = result.replace(" . ", ".");
    result = result.replace(" .", ".");
    // Collapse space around `!`: `ident ! ` → `ident!`
    result = result.replace(" ! ", "!");
    result = result.replace(" !", "!");
    // Collapse `& ident` → `&ident`
    result = result.replace("& ", "&");
    // Collapse `:: ident` → `::ident`
    result = result.replace(":: ", "::");
    // Collapse `( ` → `(`  and  ` )` → `)`  (space around parens)
    result = result.replace("( ", "(");
    result = result.replace(" )", ")");
    // Collapse `[ ` → `[`  and  ` ]` → `]`  (space around brackets)
    result = result.replace("[ ", "[");
    result = result.replace(" ]", "]");
    // Collapse ` , ` → `, `  and  ` ; ` → `; ` (space before separator)
    result = result.replace(" ,", ",");
    result = result.replace(" ;", ";");
    // Collapse ` : ` → `: ` (space before colon in repetition bodies like `$arg: $ty`)
    result = result.replace(" : ", ": ");
    result
}

/// Normalize spacing in multi-line restored arm body text.
/// Handles spacing around `.`, `!`, `&`, `::`, `,`, `;`, `(`, `)` but
/// NOT around `{}`, `[]` (those are context-sensitive, handled for
/// single-line content by `normalize_inner_spacing`).
///
/// Also protects ` )+`, ` )*`, ` )?` (repetition closers) from the
/// `" )"` → `")"` rule, which would break layout inside $()...)+ blocks.
fn normalize_body_spacing(text: &str) -> String {
    transform_outside_literals_and_comments(text, normalize_body_spacing_raw)
}

fn normalize_body_spacing_raw(text: &str) -> String {
    let mut result = text.to_string();
    // Protect repetition closers: ` )+`, ` )*`, ` )?`
    result = result.replace(" )+", "\x00RP\x00");
    result = result.replace(" )*", "\x00RS\x00");
    result = result.replace(" )?", "\x00RQ\x00");
    // Collapse space before a lone dot: ` . ` → `.`
    result = result.replace(" . ", ".");
    result = result.replace(" .", ".");
    // Collapse space around `!`: `ident ! ` → `ident!`
    result = result.replace(" ! ", "!");
    result = result.replace(" !", "!");
    // Collapse `& ident` → `&ident`
    result = result.replace("& ", "&");
    // Collapse `:: ident` → `::ident`
    result = result.replace(":: ", "::");
    // Collapse ` , ` → `, `  and  ` ; ` → `; ` (space before separator)
    result = result.replace(" ,", ",");
    // Collapse ` : ` → `: ` (space before colon inside macro repetition bodies)
    result = result.replace(" : ", ": ");
    // Collapse space after `(` and before `)`: these are safe because
    // rustfmt never produces `( ` or ` )` in valid code — they only
    // appear inside unformatted macro invocations.
    result = result.replace("( ", "(");
    result = result.replace(" )", ")");
    result = result.replace("[ ", "[");
    result = result.replace(" ]", "]");
    // Remove space between a metavar and `(`: rustfmt often adds a space
    // before `(` after an ident/macro-name (e.g., `$name ($arg)` → `$name($arg)`).
    result = result.replace("$ (", "$(");
    result = result.replace("), *", "),*");
    result = result.replace("), +", "),+");
    result = result.replace("), ?", "),?");
    result = result.replace("); *", ");*");
    result = result.replace("); +", ");+");
    // Restore repetition closers
    result = result.replace("\x00RP\x00", " )+");
    result = result.replace("\x00RS\x00", " )*");
    result = result.replace("\x00RQ\x00", " )?");
    result
}

/// Replace `__mf_rep_{kind}! { inner }` markers with `$(inner){char}{sep}`.
/// Handles nested markers recursively.
fn replace_rep_markers(text: &str, repetition_prefix: &str) -> String {
    let mut result = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with(repetition_prefix) {
            let marker_start = i;
            let kind_start = i + repetition_prefix.len();
            let rest = &text[kind_start..];
            let kind_end = rest.find('!').unwrap_or(rest.len());
            let kind = &rest[..kind_end];
            let (rep_char, sep) = match kind {
                "star" => ('*', None),
                "plus" => ('+', None),
                "question" => ('?', None),
                "star_comma" => ('*', Some(',')),
                "plus_comma" => ('+', Some(',')),
                "star_semi" => ('*', Some(';')),
                "plus_semi" => ('+', Some(';')),
                _ => {
                    // Not a valid marker, push as-is
                    result.push_str(repetition_prefix);
                    i = kind_start;
                    continue;
                }
            };
            let after_kind = &text[kind_start + kind_end..];
            if let Some(brace_rel) = after_kind.find('{') {
                let brace_pos = kind_start + kind_end + brace_rel;
                let mut depth = 1;
                let mut close_pos = brace_pos + 1;
                while close_pos < bytes.len() && depth > 0 {
                    match bytes[close_pos] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        _ => {}
                    }
                    close_pos += 1;
                }
                if depth == 0 {
                    // Extract inner and recursively process
                    let inner = &text[brace_pos + 1..close_pos - 1];
                    let inner_replaced = replace_rep_markers(inner, repetition_prefix);
                    // Normalize spacing in inner content
                    let compact = canonical_token_spacing(&inner_replaced);
                    let inner_final = if inner_replaced.contains('\n')
                        && compact.len() <= 80
                        && !compact.contains(['{', '}', ';'])
                        && !compact.contains("$(")
                        && !contains_comment(&inner_replaced)
                    {
                        compact
                    } else if inner_replaced.contains('\n') {
                        inner_replaced
                    } else {
                        normalize_inner_spacing(&inner_replaced)
                    };
                    let nested_block = kind == "question"
                        && inner_final.contains("$(")
                        && (inner_final.contains(": ")
                            || inner_final.contains("pub ")
                            || inner_final.contains(';')
                            || ["struct ", "enum ", "impl ", "fn ", "trait ", "mod "]
                                .iter()
                                .any(|keyword| inner_final.contains(keyword)));
                    if inner_final.contains('\n') || inner_final.contains('\r') || nested_block {
                        // Multi-line: re-indent relative to marker position
                        let line_start =
                            text[..marker_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
                        let marker_indent = marker_start - line_start;
                        let base_indent = marker_indent + 4;
                        // Find minimum indent within inner content
                        let min_indent = inner_final
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .map(|l| l.len() - l.trim_start().len())
                            .min()
                            .unwrap_or(0);
                        let mut indented = String::new();
                        for line in inner_final.lines() {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                indented.push('\n');
                            } else {
                                let line_indent = line.len() - line.trim_start().len();
                                let total_indent =
                                    base_indent + line_indent.saturating_sub(min_indent);
                                indented.push_str(&" ".repeat(total_indent));
                                indented.push_str(trimmed);
                                indented.push('\n');
                            }
                        }
                        // Remove trailing newline
                        let indented = indented.trim_end_matches('\n');
                        result.push('$');
                        result.push('(');
                        result.push('\n');
                        result.push_str(indented);
                        result.push('\n');
                        result.push_str(&" ".repeat(marker_indent));
                        result.push(')');
                        if let Some(s) = sep {
                            result.push(s);
                        }
                        result.push(rep_char);
                    } else {
                        result.push('$');
                        result.push('(');
                        result.push_str(&inner_final);
                        result.push(')');
                        if let Some(s) = sep {
                            result.push(s);
                        }
                        result.push(rep_char);
                    }
                    i = close_pos;
                    continue;
                }
            }
            // Couldn't parse marker, push "{" as-is and continue
            result.push_str(&text[marker_start..marker_start + 10]);
            i = marker_start + 10;
        } else {
            let ch = text[i..].chars().next().expect("index is a char boundary");
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

/// Replace placeholder identifiers with original macro text.
/// Sorts by placeholder length (longest first) to avoid partial replacements.
pub(crate) fn restore_placeholders(text: &str, mapping: &Mapping) -> String {
    let mut result = text.to_string();
    let mut placeholders: Vec<(&String, &String)> = mapping.vars.iter().collect();
    // Sort by key length descending to avoid partial matches
    placeholders.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (placeholder, original) in &placeholders {
        result = result.replace(placeholder.as_str(), original.as_str());
    }
    result
}
