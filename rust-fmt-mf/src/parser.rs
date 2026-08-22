use std::ops::Range;

use anyhow::{bail, ensure};
use ra_ap_rustc_lexer::{tokenize, FrontmatterAllowed, TokenKind};

use crate::types::{MacroArm, MacroDef};

#[derive(Clone, Debug)]
struct SourceToken {
    kind: TokenKind,
    span: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignificantToken {
    pub kind: String,
    pub text: String,
    pub span: Range<usize>,
}

fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    )
}

fn lex(source: &str) -> anyhow::Result<Vec<SourceToken>> {
    let mut offset = 0usize;
    let mut tokens = Vec::new();
    for token in tokenize(source, FrontmatterAllowed::Yes) {
        let len = token.len as usize;
        let end = offset
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("Rust token offset overflow"))?;
        ensure!(
            source.is_char_boundary(offset) && source.is_char_boundary(end),
            "Rust lexer returned a non-UTF-8 token boundary at {offset}..{end}"
        );
        tokens.push(SourceToken {
            kind: token.kind,
            span: offset..end,
        });
        offset = end;
    }
    ensure!(
        offset == source.len(),
        "Rust lexer stopped at byte {offset} of {}",
        source.len()
    );
    Ok(tokens)
}

pub fn significant_tokens(source: &str) -> anyhow::Result<Vec<SignificantToken>> {
    Ok(lex(source)?
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace))
        .map(|token| SignificantToken {
            kind: format!("{:?}", token.kind),
            text: source[token.span.clone()].to_string(),
            span: token.span,
        })
        .collect())
}

fn next_non_trivia(tokens: &[SourceToken], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|&index| !is_trivia(tokens[index].kind))
}

fn delimiter_pair(kind: TokenKind) -> Option<(TokenKind, TokenKind)> {
    match kind {
        TokenKind::OpenParen => Some((TokenKind::OpenParen, TokenKind::CloseParen)),
        TokenKind::OpenBrace => Some((TokenKind::OpenBrace, TokenKind::CloseBrace)),
        TokenKind::OpenBracket => Some((TokenKind::OpenBracket, TokenKind::CloseBracket)),
        _ => None,
    }
}

fn matching_delimiter(tokens: &[SourceToken], open: usize) -> anyhow::Result<usize> {
    let Some((open_kind, close_kind)) = delimiter_pair(tokens[open].kind) else {
        bail!(
            "token at {} is not an opening delimiter",
            tokens[open].span.start
        );
    };
    let mut stack = vec![close_kind];
    for (index, token) in tokens.iter().enumerate().skip(open + 1) {
        if let Some((_, expected_close)) = delimiter_pair(token.kind) {
            stack.push(expected_close);
            continue;
        }
        if matches!(
            token.kind,
            TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket
        ) {
            let expected = stack.pop().ok_or_else(|| {
                anyhow::anyhow!("unexpected closing delimiter at byte {}", token.span.start)
            })?;
            ensure!(
                token.kind == expected,
                "mismatched delimiter at byte {}: expected {:?}, found {:?}",
                token.span.start,
                expected,
                token.kind
            );
            if stack.is_empty() {
                return Ok(index);
            }
        }
    }
    bail!(
        "unclosed {:?} delimiter at byte {}",
        open_kind,
        tokens[open].span.start
    )
}

fn scan_arms(tokens: &[SourceToken], open: usize, close: usize) -> anyhow::Result<Vec<MacroArm>> {
    let mut arms = Vec::new();
    let mut cursor = open + 1;
    while let Some(pattern_open) = next_non_trivia(tokens, cursor) {
        if pattern_open >= close {
            break;
        }
        if matches!(
            tokens[pattern_open].kind,
            TokenKind::Semi | TokenKind::Comma
        ) {
            cursor = pattern_open + 1;
            continue;
        }
        if delimiter_pair(tokens[pattern_open].kind).is_none() {
            cursor = pattern_open + 1;
            continue;
        }

        let pattern_close = matching_delimiter(tokens, pattern_open)?;
        ensure!(
            pattern_close < close,
            "macro matcher crosses its definition boundary"
        );
        let eq = next_non_trivia(tokens, pattern_close + 1)
            .ok_or_else(|| anyhow::anyhow!("macro matcher has no transcriber"))?;
        let gt = next_non_trivia(tokens, eq + 1)
            .ok_or_else(|| anyhow::anyhow!("macro matcher has incomplete fat arrow"))?;
        ensure!(
            tokens[eq].kind == TokenKind::Eq
                && tokens[gt].kind == TokenKind::Gt
                && tokens[eq].span.end == tokens[gt].span.start,
            "expected fat arrow after macro matcher at byte {}",
            tokens[pattern_close].span.end
        );
        let body_open = next_non_trivia(tokens, gt + 1)
            .ok_or_else(|| anyhow::anyhow!("macro arm has no transcriber body"))?;
        ensure!(
            delimiter_pair(tokens[body_open].kind).is_some(),
            "macro transcriber at byte {} is not delimited",
            tokens[body_open].span.start
        );
        let body_close = matching_delimiter(tokens, body_open)?;
        ensure!(
            body_close < close,
            "macro transcriber crosses its definition boundary"
        );
        arms.push(MacroArm {
            pattern_span: tokens[pattern_open].span.start..tokens[pattern_close].span.end,
            body_span: tokens[body_open].span.start..tokens[body_close].span.end,
        });
        cursor = body_close + 1;
    }
    Ok(arms)
}

/// Parse macro_rules definitions using Rust lexer tokens and byte-accurate spans.
pub fn parse_macro_defs(source: &str) -> anyhow::Result<Vec<MacroDef>> {
    let tokens = lex(source)?;
    let mut defs = Vec::new();

    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Ident
            || &source[tokens[index].span.clone()] != "macro_rules"
        {
            continue;
        }
        let Some(bang) = next_non_trivia(&tokens, index + 1) else {
            continue;
        };
        if tokens[bang].kind != TokenKind::Bang {
            continue;
        }
        let Some(name_index) = next_non_trivia(&tokens, bang + 1) else {
            continue;
        };
        if !matches!(
            tokens[name_index].kind,
            TokenKind::Ident | TokenKind::RawIdent
        ) {
            continue;
        }
        let Some(open) = next_non_trivia(&tokens, name_index + 1) else {
            continue;
        };
        if delimiter_pair(tokens[open].kind).is_none() {
            continue;
        }
        let close = matching_delimiter(&tokens, open)?;
        let arms = scan_arms(&tokens, open, close)?;
        if arms.is_empty() {
            continue;
        }

        let line_start = source[..tokens[index].span.start]
            .rfind('\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        let start = find_attr_span_start(source, line_start);
        let mut end = tokens[close].span.end;
        if let Some(semi) = next_non_trivia(&tokens, close + 1) {
            if tokens[semi].kind == TokenKind::Semi {
                end = tokens[semi].span.end;
            }
        }
        defs.push(MacroDef {
            name: source[tokens[name_index].span.clone()].to_string(),
            span: start..end,
            arms,
        });
    }

    defs.sort_by(|left, right| right.span.len().cmp(&left.span.len()));
    let mut outer = Vec::new();
    for definition in defs {
        if !outer.iter().any(|parent: &MacroDef| {
            parent.span.start <= definition.span.start && parent.span.end >= definition.span.end
        }) {
            outer.push(definition);
        }
    }
    outer.sort_by_key(|definition| definition.span.start);
    Ok(outer)
}

fn find_attr_span_start(source: &str, pos: usize) -> usize {
    let mut current = pos;
    let mut found_attribute = false;
    loop {
        if current == 0 {
            return if found_attribute { 0 } else { pos };
        }
        let Some(previous_newline) = source[..current].rfind('\n') else {
            return if found_attribute { 0 } else { pos };
        };
        let line_start = source[..previous_newline]
            .rfind('\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        let trimmed = source[line_start..previous_newline].trim();
        if trimmed.is_empty() {
            current = line_start;
            continue;
        }
        if trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || (trimmed.starts_with("#[") && !trimmed.starts_with("#!["))
            || trimmed.ends_with(']')
        {
            found_attribute = true;
            current = line_start;
            continue;
        }
        return if found_attribute { current } else { pos };
    }
}
