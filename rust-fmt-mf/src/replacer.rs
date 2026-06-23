use crate::types::Mapping;
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, TokenStream, TokenTree};

/// Replace macro syntax in a token stream with valid Rust.
///
/// Uses while-let to pass the same iterator to `$` handlers, so the
/// iterator continues from after the `$` token — not from the start.
pub fn replace_macro_syntax(tokens: &TokenStream, mapping: &mut Mapping) -> TokenStream {
    let mut result = TokenStream::new();
    let mut iter = tokens.clone().into_iter();
    while let Some(tree) = iter.next() {
        match tree {
            TokenTree::Group(group) => {
                let replaced = replace_macro_syntax(&group.stream(), mapping);
                let mut new_group = Group::new(group.delimiter(), replaced);
                new_group.set_span(group.span());
                result.extend(Some(TokenTree::Group(new_group)));
            }
            // Handle $var, $var:type, $(...), $crate
            TokenTree::Punct(punct) if punct.as_char() == '$' => {
                result.extend(replace_dollar_sequence(&mut iter, mapping));
            }
            other => {
                result.extend(Some(other));
            }
        }
    }
    result
}

/// Handle sequences starting with `$`:
///   $var         → __m_N
///   $var:type    → __m_N
///   $( ... )*    → __mf_rep_star! { ... }
///   $( ... )+    → __mf_rep_plus! { ... }
///   $( ... )?    → __mf_rep_question! { ... }
///   $( ... ),*   → __mf_rep_star_comma! { ... }
///   $( ... ),+   → __mf_rep_plus_comma! { ... }
///   $( ... );*   → __mf_rep_star_semi! { ... }
///   $( ... );+   → __mf_rep_plus_semi! { ... }
///   $crate       → __m_N (registered in Mapping)
fn replace_dollar_sequence(
    iter: &mut impl Iterator<Item = TokenTree>,
    mapping: &mut Mapping,
) -> TokenStream {
    // Peek at next token after $
    let next = match iter.next() {
        Some(t) => t,
        None => return TokenStream::new(),
    };
    match next {
        // $( ... ) — repetition group
        TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis => {
            let inner = replace_macro_syntax(&group.stream(), mapping);
            // Look ahead: [separator] (*|+)
            match iter.next() {
                // $(...)* — no separator, star repetition
                Some(TokenTree::Punct(p)) if p.as_char() == '*' => {
                    let mut ts = TokenStream::new();
                    ts.extend(quote::quote! {
                        __mf_rep_star ! { #inner }
                    });
                    ts
                }
                // $(...)+ — no separator, plus repetition
                Some(TokenTree::Punct(p)) if p.as_char() == '+' => {
                    let mut ts = TokenStream::new();
                    ts.extend(quote::quote! {
                        __mf_rep_plus ! { #inner }
                    });
                    ts
                }
                // $(...)? — no separator, optional repetition
                Some(TokenTree::Punct(p)) if p.as_char() == '?' => {
                    let mut ts = TokenStream::new();
                    ts.extend(quote::quote! {
                        __mf_rep_question ! { #inner }
                    });
                    ts
                }
                // $(...),* / $(...),+ / $(...);* / $(...);+ — with separator
                Some(TokenTree::Punct(sep)) if sep.as_char() == ',' || sep.as_char() == ';' => {
                    let sep_char = sep.as_char();
                    match iter.next() {
                        Some(TokenTree::Punct(p)) if p.as_char() == '*' => {
                            let macro_name = if sep_char == ',' {
                                "__mf_rep_star_comma"
                            } else {
                                "__mf_rep_star_semi"
                            };
                            let macro_ident = Ident::new(macro_name, sep.span());
                            let mut ts = TokenStream::new();
                            ts.extend(quote::quote! {
                                #macro_ident ! { #inner }
                            });
                            ts
                        }
                        Some(TokenTree::Punct(p)) if p.as_char() == '+' => {
                            let macro_name = if sep_char == ',' {
                                "__mf_rep_plus_comma"
                            } else {
                                "__mf_rep_plus_semi"
                            };
                            let macro_ident = Ident::new(macro_name, sep.span());
                            let mut ts = TokenStream::new();
                            ts.extend(quote::quote! {
                                #macro_ident ! { #inner }
                            });
                            ts
                        }
                        other => {
                            let mut ts = TokenStream::new();
                            ts.extend(inner);
                            ts.extend(Some(TokenTree::Punct(sep)));
                            if let Some(t) = other {
                                ts.extend(Some(t));
                            }
                            ts
                        }
                    }
                }
                other => {
                    // $( without */+ — expand as just the inner content
                    let mut ts = TokenStream::new();
                    ts.extend(inner);
                    if let Some(t) = other {
                        ts.extend(Some(t));
                    }
                    ts
                }
            }
        }
        // $crate → __mf_crate_N (unique placeholder for accurate restoration)
        TokenTree::Ident(ident) => {
            let ident_str = ident.to_string();
            if ident_str == "crate" {
                let placeholder = mapping.register("$crate");
                let ident = Ident::new(&placeholder, ident.span());
                let mut ts = TokenStream::new();
                ts.extend(Some(TokenTree::Ident(ident)));
                return ts;
            }
            // $var or $var:type
            let name = ident_str;
            // Check for :type suffix
            let next_next = iter.next();
            if let Some(TokenTree::Punct(p)) = &next_next {
                if p.as_char() == ':' {
                    // $var:type — register entire pattern
                    let type_str = collect_type(iter);
                    let original = format!("${}:{}", name, type_str);
                    let placeholder = mapping.register(&original);
                    let t = Ident::new(&placeholder, ident.span());
                    let mut ts = TokenStream::new();
                    ts.extend(Some(TokenTree::Ident(t)));
                    return ts;
                }
            }
            // Plain $var
            let original = format!("${}", name);
            let placeholder = mapping.register(&original);
            let t = Ident::new(&placeholder, ident.span());
            let mut ts = TokenStream::new();
            ts.extend(Some(TokenTree::Ident(t)));
            // Emit the peeked token if we didn't consume it as part of $var:type.
            // If it's a Group, recursively process its contents first.
            if let Some(next_token) = next_next {
                let processed = match next_token {
                    TokenTree::Group(g) => {
                        let inner = replace_macro_syntax(&g.stream(), mapping);
                        let mut new_g = Group::new(g.delimiter(), inner);
                        new_g.set_span(g.span());
                        TokenTree::Group(new_g)
                    }
                    other => other,
                };
                ts.extend(Some(processed));
            }
            ts
        }
        // $$ — literal dollar sign (rare), pass through unchanged
        TokenTree::Punct(p) if p.as_char() == '$' => {
            let mut ts = TokenStream::new();
            ts.extend(Some(TokenTree::Punct(Punct::new('$', Spacing::Alone))));
            ts.extend(Some(TokenTree::Punct(p)));
            ts
        }
        other => {
            // Not a recognized $ sequence — emit raw $ + next token
            let mut ts = TokenStream::new();
            ts.extend(Some(TokenTree::Punct(Punct::new('$', Spacing::Alone))));
            ts.extend(Some(other));
            ts
        }
    }
}

/// Collect type tokens (after `:`) until we see `,` or `;`.
fn collect_type(iter: &mut impl Iterator<Item = TokenTree>) -> String {
    let mut result = String::new();
    while let Some(t) = iter.next() {
        match &t {
            TokenTree::Punct(p) if p.as_char() == ',' || p.as_char() == ';' => {
                // Put the separator back — it belongs to the outer context
                // (e.g., $(...),* repetition separator)
                break;
            }
            _ => {
                result.push_str(&t.to_string());
            }
        }
    }
    result
}

/// Replace macro syntax in source body text, preserving original newlines
/// and whitespace structure.
///
/// Unlike `replace_macro_syntax` which works with TokenStream (losing
/// whitespace/newlines), this function operates on the raw source text
/// and preserves all original formatting around `$()` markers.
///
/// Supports:
/// - `$identifier:type` → `__m_N`
/// - `$(...)sep*`/`$(...)sep+`/`$(...)?` → `__mf_rep_*!{ ... }` (preserves surrounding whitespace)
/// - `$crate` → `__m_N`
/// - `$$` → `$`
pub fn replace_macro_syntax_text(body_text: &str, mapping: &mut Mapping) -> String {
    let mut result = String::new();
    let bytes = body_text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip strings
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            result.push_str(&body_text[start..i]);
            continue;
        }
        // Skip line comments
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            result.push_str(&body_text[start..i]);
            continue;
        }
        // Skip block comments
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2;
            }
            result.push_str(&body_text[start..i]);
            continue;
        }
        if bytes[i] == b'$' {
            // $$ → literal $ (preserve both dollar signs)
            if i + 1 < bytes.len() && bytes[i + 1] == b'$' {
                result.push_str("$$");
                i += 2;
                continue;
            }
            // $(...) — repetition group
            if i + 1 < bytes.len() && bytes[i + 1] == b'(' {
                // Find matching ')' for $(...)
                let mut paren_depth = 1u32;
                let mut j = i + 2;
                while j < bytes.len() && paren_depth > 0 {
                    match bytes[j] {
                        b'(' => paren_depth += 1,
                        b')' => paren_depth -= 1,
                        b'"' => {
                            // Skip string inside pattern
                            j += 1;
                            while j < bytes.len() && bytes[j] != b'"' {
                                if bytes[j] == b'\\' {
                                    j += 1;
                                }
                                j += 1;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                if paren_depth != 0 {
                    result.push('$');
                    i += 1;
                    continue;
                }
                let close_paren = j - 1; // position of ')'
                // Extract inner pattern (between $( and ))
                let inner = &body_text[i + 2..close_paren];
                // Recursively replace macro syntax in the inner pattern
                let inner_replaced = replace_macro_syntax_text(inner, mapping);
                // Look for separator and repetition character after ')'
                let mut k = close_paren + 1;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                let mut sep = None;
                let mut rep = None;
                if k < bytes.len() && (bytes[k] == b',' || bytes[k] == b';') {
                    sep = Some(bytes[k] as char);
                    k += 1;
                    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                        k += 1;
                    }
                }
                if k < bytes.len() && (bytes[k] == b'*' || bytes[k] == b'+' || bytes[k] == b'?') {
                    rep = Some(bytes[k] as char);
                    k += 1;
                }
                let marker_name = match (rep, sep) {
                    (Some('*'), None) => "__mf_rep_star",
                    (Some('+'), None) => "__mf_rep_plus",
                    (Some('?'), None) => "__mf_rep_question",
                    (Some('*'), Some(',')) => "__mf_rep_star_comma",
                    (Some('+'), Some(',')) => "__mf_rep_plus_comma",
                    (Some('*'), Some(';')) => "__mf_rep_star_semi",
                    (Some('+'), Some(';')) => "__mf_rep_plus_semi",
                    _ => {
                        // Not a valid repetition — emit as-is
                        result.push_str("$(");
                        result.push_str(&inner_replaced);
                        result.push(')');
                        i = close_paren + 1;
                        continue;
                    }
                };
                // Preserve whitespace between the previous token and the marker
                // by copying the text before '$(' up to the marker
                result.push_str(marker_name);
                result.push_str("!{");
                result.push_str(&inner_replaced);
                result.push('}');
                i = k;
                continue;
            }
            // $identifier — macro variable
            if i + 1 < bytes.len() && (bytes[i + 1].is_ascii_alphanumeric() || bytes[i + 1] == b'_')
            {
                let id_start = i + 1;
                let mut id_end = id_start;
                while id_end < bytes.len()
                    && (bytes[id_end].is_ascii_alphanumeric() || bytes[id_end] == b'_')
                {
                    id_end += 1;
                }
                let ident = &body_text[id_start..id_end];
                if ident == "crate" {
                    // $crate → __m_N
                    let placeholder = mapping.register("$crate");
                    result.push_str(&placeholder);
                    i = id_end;
                    continue;
                }
                // Check for $var:type
                let mut type_pos = id_end;
                while type_pos < bytes.len() && bytes[type_pos] == b' ' {
                    type_pos += 1;
                }
                if type_pos < bytes.len() && bytes[type_pos] == b':' {
                    // $var:type — but only if there's an actual type name
                    let mut type_end = type_pos + 1;
                    while type_end < bytes.len()
                        && (bytes[type_end].is_ascii_alphanumeric() || bytes[type_end] == b'_')
                    {
                        type_end += 1;
                    }
                    // If the character after ':' wasn't alphanumeric, this is
                    // not a type annotation — it's a literal colon in the body
                    // (e.g. `$field: $ty` in a struct definition).
                    if type_end > type_pos + 1 {
                        let type_str = &body_text[type_pos + 1..type_end];
                        let original = format!("${}:{}", ident, type_str);
                        let placeholder = mapping.register(&original);
                        result.push_str(&placeholder);
                        i = type_end;
                        continue;
                    }
                }
                // Plain $var
                let original = format!("${}", ident);
                let placeholder = mapping.register(&original);
                result.push_str(&placeholder);
                i = id_end;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}


