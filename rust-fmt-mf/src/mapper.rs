use crate::types::{MacroDef, Mapping};

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
    let mut result = String::with_capacity(original_source.len());
    let mut src_pos = 0;
    // Split the shared shadow file into all arm sections upfront
    let all_arm_sections = split_shadow_into_arms(formatted_shadow);
    let mut mapping_offset = 0;
    let mut section_offset = 0;
    for def in macro_defs.iter() {
        result.push_str(&original_source[src_pos..def.span.start]);
        // Walk through the macro source, replacing each arm body position
        let mut macro_pos = def.span.start;
        for arm_idx in 0..def.arms.len() {
            if section_offset + arm_idx >= all_arm_sections.len() {
                let _body_start = def.arms[arm_idx].body_span.start;
                let body_end = def.arms[arm_idx].body_span.end;
                result.push_str(&original_source[macro_pos..body_end]);
                macro_pos = body_end;
                continue;
            }
            let arm = &def.arms[arm_idx];
            let mapping = &all_mappings[mapping_offset + arm_idx];
            let section = &all_arm_sections[section_offset + arm_idx];
            let body_start = arm.body_span.start;
            let body_end = arm.body_span.end;
            // Calculate indent from the macro_rules! line. With span.start now
            // including leading whitespace, count the whitespace directly.
            let macro_indent = original_source[def.span.start..]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            let brace_indent = macro_indent + 4;
            // Copy source from last position up to the `{`, normalizing pattern spacing
            let pattern_text = &original_source[macro_pos..body_start];
            let normalized_pattern = normalize_pattern_text(pattern_text);
            // Collapse multi-line patterns to single line (rustfmt preserves
            // newlines from the original source — we want them on one line).
            let mut collapsed_pattern = collapse_pattern_newlines(&normalized_pattern);
            // Normalize spacing around parens, brackets, braces, commas, and semicolons in patterns.
            // Run AFTER collapsing so the replacements operate on single-line text.
            // Loop to converge multi-space patterns (e.g. `[   $x:expr   ]` → `[$x:expr]`).
            //
            // IMPORTANT: For the first arm, `pattern_text` includes the macro header prefix
            // `macro_rules! name {`. Find the first `{` (the macro body delimiter) and
            // normalize only the suffix after it, to avoid removing the space before it.
            // For subsequent arms the pattern text has no macro header, so normalize all of it.
            let is_first_arm = macro_pos == def.span.start;
            if is_first_arm {
                if let Some(open_pos) = collapsed_pattern.find('{') {
                    let prefix = &collapsed_pattern[..=open_pos];
                    let suffix = &collapsed_pattern[open_pos + 1..];
                    let mut normalized_suffix = suffix.to_string();
                    let mut prev: String;
                    loop {
                        prev = normalized_suffix.clone();
                        normalized_suffix = normalized_suffix
                            .replace(" (", "(")
                            .replace("( ", "(")
                            .replace(" )", ")")
                            .replace(" [", "[")
                            .replace("[ ", "[")
                            .replace(" ]", "]")
                            .replace(" {", "{")
                            .replace("{ ", "{")
                            .replace(" }", "}")
                            .replace(" ,", ",")
                            .replace(" ;", ";");
                        if normalized_suffix == prev {
                            break;
                        }
                    }
                    collapsed_pattern = format!("{}{}", prefix, normalized_suffix);
                } else {
                    let mut prev: String;
                    loop {
                        prev = collapsed_pattern.clone();
                        collapsed_pattern = collapsed_pattern
                            .replace(" (", "(")
                            .replace("( ", "(")
                            .replace(" )", ")")
                            .replace(" [", "[")
                            .replace("[ ", "[")
                            .replace(" ]", "]")
                            .replace(" {", "{")
                            .replace("{ ", "{")
                            .replace(" }", "}")
                            .replace(" ,", ",")
                            .replace(" ;", ";");
                        if collapsed_pattern == prev {
                            break;
                        }
                    }
                }
            } else {
                let mut prev: String;
                loop {
                    prev = collapsed_pattern.clone();
                    collapsed_pattern = collapsed_pattern
                        .replace(" (", "(")
                        .replace("( ", "(")
                        .replace(" )", ")")
                        .replace(" [", "[")
                        .replace("[ ", "[")
                        .replace(" ]", "]")
                        .replace(" {", "{")
                        .replace("{ ", "{")
                        .replace(" }", "}")
                        .replace(" ,", ",")
                        .replace(" ;", ";");
                    if collapsed_pattern == prev {
                        break;
                    }
                }
            }
            // Re-indent the pattern line to match brace_indent
            if let Some(last_nl) = collapsed_pattern.rfind('\n') {
                let prefix = &collapsed_pattern[..=last_nl];
                let pattern_line = &collapsed_pattern[last_nl + 1..];
                let trimmed = pattern_line.trim_start();
                result.push_str(prefix);
                result.push_str(&" ".repeat(brace_indent));
                result.push_str(trimmed);
            } else {
                // No prefix newline — single-line context
                let trimmed = collapsed_pattern.trim_start();
                result.push_str(&" ".repeat(brace_indent));
                result.push_str(trimmed);
            }
            let formatted_inner = map_arm_section(section, mapping);
            // Emit `{` (or `{{` for double_brace) and newline
            let is_double_brace = original_source[body_start..].starts_with("{{");
            if formatted_inner.trim().is_empty() {
                // Empty body: emit `{}` on the same line as the pattern
                if is_double_brace {
                    result.push_str("{{}}");
                } else {
                    result.push_str("{}");
                }
            } else {
                if is_double_brace {
                    result.push_str("{{\n");
                } else {
                    result.push('{');
                    result.push('\n');
                }
                // Re-indent: find minimum indent in inner lines, map to brace_indent + 4
                let min_indent = formatted_inner
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.len() - l.trim_start().len())
                    .min()
                    .unwrap_or(0);
                let base_indent = brace_indent + 4;
                for line in formatted_inner.lines() {
                    let trimmed = line.trim_start();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let line_indent = line.len() - trimmed.len();
                    let total_indent = base_indent + line_indent.saturating_sub(min_indent);
                    result.push_str(&" ".repeat(total_indent));
                    result.push_str(trimmed);
                    result.push('\n');
                }
                // Emit closing `}` (or `}}` for double_brace)
                result.push_str(&" ".repeat(brace_indent));
                if is_double_brace {
                    result.push_str("}}");
                } else {
                    result.push('}');
                }
            }
            macro_pos = body_end;
        }
        mapping_offset += def.arms.len();
        section_offset += def.arms.len();
        // Copy remaining content after last arm body (semicolons, closing braces)
        result.push_str(&original_source[macro_pos..def.span.end]);
        src_pos = def.span.end;
    }
    // Copy remaining code after last macro
    if src_pos < original_source.len() {
        result.push_str(&original_source[src_pos..]);
    }
    result
}

/// Split the formatted shadow file into individual arm body sections.
///
/// Each arm is `macro_rules! __rustfmt_mf_arm_N { () => { BODY }; }`.
/// We extract just the BODY content (inside the `{}` after `=>`).
pub(crate) fn split_shadow_into_arms(shadow_file: &str) -> Vec<String> {
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

/// Normalize `$(...)` spacing in pattern text (macro arm patterns).
/// Scans for `$(` and normalizes spacing around delimiters:
///   `$ (` → `$(`  `( x )` → `(x)`  `) *` → `)*`  `) ,` → `),`
fn normalize_pattern_text(text: &str) -> String {
    let mut result = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'(' {
                result.push('$');
                result.push('(');
                let mut depth = 1;
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_ascii_whitespace() && bytes[k] != b'\n' {
                    k += 1;
                }
                let inner_start = k;
                while k < bytes.len() && depth > 0 {
                    if bytes[k] == b'(' {
                        depth += 1;
                    }
                    if bytes[k] == b')' {
                        depth -= 1;
                    }
                    if depth > 0 {
                        k += 1;
                    }
                }
                let inner_end = k;
                if depth == 0 {
                    let inner = &text[inner_start..inner_end];
                    let inner_norm = normalize_pattern_text(inner);
                    result.push_str(inner_norm.trim());
                    result.push(')');
                    k += 1;
                    while k < bytes.len() && bytes[k].is_ascii_whitespace() && bytes[k] != b'\n' {
                        k += 1;
                    }
                    if k < bytes.len() && (bytes[k] == b',' || bytes[k] == b';') {
                        result.push(bytes[k] as char);
                        k += 1;
                    }
                    if k < bytes.len() && (bytes[k] == b'*' || bytes[k] == b'+' || bytes[k] == b'?')
                    {
                        result.push(bytes[k] as char);
                        k += 1;
                    }
                    i = k;
                    continue;
                }
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Collapse multi-line arm patterns to a single line.
/// Preserves the `macro_rules! name {` header and its first
/// newline+whitespace (the indent before the arm pattern).
/// Only collapses newlines within the arm pattern itself.
fn collapse_pattern_newlines(text: &str) -> String {
    // Find the macro opening `{` (the first `{` that isn't inside $(...) )
    let mut in_dollar_paren = false;
    let mut macro_open = None;
    let bytes = text.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if b == b'$' && idx + 1 < bytes.len() && bytes[idx + 1] == b'(' {
            in_dollar_paren = true;
            continue;
        }
        if in_dollar_paren {
            if b == b')' {
                in_dollar_paren = false;
            }
            continue;
        }
        if b == b'{' {
            macro_open = Some(idx);
            break;
        }
    }
    if let Some(open_pos) = macro_open {
        let prefix = &text[..=open_pos];
        let rest = &text[open_pos + 1..];
        // Keep the first newline+indent after `{`, but collapse everything
        // within the arm pattern (between `(` and `) => {`)
        let trimmed = rest.trim_start();
        let leading_ws = &rest[..rest.len() - trimmed.len()];
        // Only collapse newlines if there are any in the arm pattern
        if !trimmed.contains('\n') {
            return text.to_string();
        }
        let mut collapsed = String::with_capacity(trimmed.len());
        let mut prev_was_space = false;
        for ch in trimmed.chars() {
            if ch == '\n' || ch == '\r' {
                if !prev_was_space {
                    collapsed.push(' ');
                    prev_was_space = true;
                }
            } else {
                collapsed.push(ch);
                prev_was_space = ch == ' ';
            }
        }
        // Normalize multiple spaces to single
        let mut result = String::with_capacity(text.len());
        result.push_str(prefix);
        result.push_str(leading_ws);
        let mut prev_space = false;
        for ch in collapsed.chars() {
            if ch == ' ' {
                if !prev_space {
                    result.push(ch);
                    prev_space = true;
                }
            } else {
                result.push(ch);
                prev_space = false;
            }
        }
        result
    } else {
        text.to_string()
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
            result.push(bytes[i] as char);
            i += 1;
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
    let single = lines.join(" ");
    if single.len() <= 80 {
        single
    } else {
        text.to_string()
    }
}

/// Collapse a simple delimited list (tuple, bracket, block) to single line
/// if it was split by rustfmt unnecessarily (e.g. short tuple `(a, b, c)`).
fn collapse_simple_delimited(text: &str) -> String {
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
    let joined = parts.join(" ");
    // Strip trailing comma before the closing delimiter
    let joined = if joined.ends_with(',') {
        joined[..joined.len() - 1].trim_end().to_string()
    } else {
        joined
    };
    let single = format!("{}{}{}", open, joined, close);
    if single.len() <= 80 {
        single
    } else {
        text.to_string()
    }
}

/// Detect `macro_rules! __rustfmt_mf_arm_N` or `macro_rules ! __rustfmt_mf_arm_N`
fn detect_arm_opener(line: &str) -> Option<usize> {
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
fn map_arm_section(section: &str, mapping: &Mapping) -> String {
    let with_reps = replace_rep_markers(section);
    let restored = restore_placeholders(&with_reps, mapping);
    let spaced = normalize_body_spacing(&restored);
    let spaced = remove_metavar_paren_space(&spaced);
    let spaced = collapse_simple_delimited(&spaced);
    collapse_short_body(&spaced)
}

/// Normalize proc_macro2's default spacing inside a single-line
/// macro invocation body.  proc_macro2 adds spaces between every token
/// (e.g. `__m_0 . to_string ()` instead of `__m_0.to_string()`), and
/// because this sits inside `__mf_rep_*!{ … }` rustfmt never touches it.
fn normalize_inner_spacing(text: &str) -> String {
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
    // Collapse `{ ` → `{`  and  ` }` → `}`  (space around braces)
    result = result.replace("{ ", "{");
    result = result.replace(" }", "}");
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
    // Remove space between a metavar and `(`: rustfmt often adds a space
    // before `(` after an ident/macro-name (e.g., `$name ($arg)` → `$name($arg)`).
    result = result.replace("$ (", "$(");
    // Restore repetition closers
    result = result.replace("\x00RP\x00", " )+");
    result = result.replace("\x00RS\x00", " )*");
    result = result.replace("\x00RQ\x00", " )?");
    result
}

/// Replace `__mf_rep_{kind}! { inner }` markers with `$(inner){char}{sep}`.
/// Handles nested markers recursively.
fn replace_rep_markers(text: &str) -> String {
    let mut result = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with("__mf_rep_") {
            let marker_start = i;
            let kind_start = i + "__mf_rep_".len();
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
                    result.push_str("__mf_rep_");
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
                    let inner_replaced = replace_rep_markers(inner);
                    // Normalize spacing in inner content
                    let inner_final = if inner_replaced.contains('\n') {
                        inner_replaced
                    } else {
                        normalize_inner_spacing(&inner_replaced)
                    };
                    if inner_final.contains('\n') || inner_final.contains('\r') {
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
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

/// Replace placeholder identifiers with original macro text.
/// Sorts by placeholder length (longest first) to avoid partial replacements.
fn restore_placeholders(text: &str, mapping: &Mapping) -> String {
    let mut result = text.to_string();
    let mut placeholders: Vec<(&String, &String)> = mapping.vars.iter().collect();
    // Sort by key length descending to avoid partial matches
    placeholders.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (placeholder, original) in &placeholders {
        result = result.replace(placeholder.as_str(), original.as_str());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // __m_10 should be replaced before __m_1 to avoid partial match
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
}