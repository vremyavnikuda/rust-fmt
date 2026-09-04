use clap::Parser;
use std::io::{self, Read, Write};

#[derive(Parser)]
#[command(name = "rust-fmt-mf")]
#[command(about = "Format macro_rules! bodies using rustfmt")]
struct Cli {
    /// Edition to pass to rustfmt (default: 2021)
    #[arg(long, default_value = "2021")]
    edition: String,

    /// Path to rustfmt executable
    #[arg(long, default_value = "rustfmt")]
    rustfmt_path: String,

    /// Path to rustfmt.toml or .rustfmt.toml
    #[arg(long)]
    config_path: Option<String>,

    /// Delete blank lines inside braces, keeping large files compact.
    /// Off by default, matching rustfmt, which preserves them.
    #[arg(long)]
    compact_blank_lines: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    let result = rust_fmt_mf::format_source_with_report_and_options(
        &source,
        &cli.rustfmt_path,
        &cli.edition,
        cli.config_path.as_deref(),
        rust_fmt_mf::types::FormatOptions {
            compact_blank_lines: cli.compact_blank_lines,
        },
    )?;
    let mut stderr = io::stderr().lock();
    for outcome in &result.macros {
        let status = match &outcome.status {
            rust_fmt_mf::types::MacroStatus::Formatted => "FORMATTED",
            rust_fmt_mf::types::MacroStatus::Unchanged => "UNCHANGED",
            rust_fmt_mf::types::MacroStatus::Skipped { reason } => {
                writeln!(
                    stderr,
                    "rust-fmt-mf\tSKIPPED\t{}\t{}..{}\t{}",
                    outcome.name,
                    outcome.span.start,
                    outcome.span.end,
                    reason.replace(['\r', '\n', '\t'], " ")
                )?;
                continue;
            }
        };
        writeln!(
            stderr,
            "rust-fmt-mf\t{}\t{}\t{}..{}",
            status, outcome.name, outcome.span.start, outcome.span.end
        )?;
    }
    io::stdout().write_all(result.text.as_bytes())?;
    Ok(())
}
