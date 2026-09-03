use anyhow::{ensure, Context, Result};
use ra_ap_rustc_lexer::{tokenize, FrontmatterAllowed, TokenKind};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(root: &Path) -> Result<Vec<PathBuf>> {
    fn visit(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(directory)
            .with_context(|| format!("cannot read {}", directory.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                visit(&path, files)?;
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn protected_line_starts(source: &str) -> HashSet<usize> {
    let mut protected = HashSet::new();
    let mut offset = 0usize;
    for token in tokenize(source, FrontmatterAllowed::Yes) {
        let end = offset + token.len as usize;
        if !matches!(token.kind, TokenKind::Whitespace) {
            for (index, byte) in source[offset..end].bytes().enumerate() {
                if byte == b'\n' && offset + index + 1 < end {
                    protected.insert(offset + index + 1);
                }
            }
        }
        offset = end;
    }
    protected
}

fn token_signature(source: &str) -> Result<Vec<(String, String)>> {
    Ok(rust_fmt_mf::parser::significant_tokens(source)?
        .into_iter()
        .map(|token| (token.kind, token.text))
        .collect())
}

fn unformat(source: &str) -> Result<String> {
    let protected = protected_line_starts(source);
    let widths = [13usize, 3, 17, 7, 21, 1, 11];
    let mut output = String::with_capacity(source.len() * 2);
    let mut offset = 0usize;
    let mut changed_lines = 0usize;
    for segment in source.split_inclusive('\n') {
        let (line, newline) = segment
            .strip_suffix('\n')
            .map_or((segment, ""), |line| (line, "\n"));
        let trimmed = line.trim_start_matches([' ', '\t']);
        let can_change = !trimmed.is_empty() && !protected.contains(&offset);
        if can_change {
            output.push_str(&" ".repeat(widths[changed_lines % widths.len()]));
            output.push_str(trimmed);
            changed_lines += 1;
        } else {
            output.push_str(line);
        }
        output.push_str(newline);
        if can_change
            && !newline.is_empty()
            && offset + segment.len() < source.len()
            && trimmed.ends_with(';')
            && !trimmed.starts_with("//")
        {
            output.push('\n');
        }
        offset += segment.len();
    }
    ensure!(changed_lines > 0, "source contains no lines to unformat");
    ensure!(source!= output, "unformatter did not change the source");
    ensure!(
        token_signature(source)? == token_signature(&output)?, "unformatter changed Rust tokens"
    );
    Ok(output)
}

fn main() -> Result<()> {
    let mut arguments = env::args_os().skip(1);
    let source_root = PathBuf::from(arguments.next().context("missing source directory")?);
    let target_root = PathBuf::from(arguments.next().context("missing target directory")?);
    ensure!(arguments.next().is_none(), "expected: <source-dir> <target-dir>");
    let files = rust_files(&source_root)?;
    ensure!(!files.is_empty(), "no Rust files found");
    for source_path in files {
        let relative = source_path.strip_prefix(&source_root)?;
        let target_path = target_root.join(relative);
        let source = fs::read_to_string(&source_path)?;
        let unformatted = unformat(&source)
            .with_context(|| format!("cannot unformat {}", source_path.display()))?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target_path, unformatted)?;
        println!("{}", relative.display());
    }
    Ok(())
}
