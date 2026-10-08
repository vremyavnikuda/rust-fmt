use clap::Parser;
use std::io::{self, Read, Write};
use std::process::ExitCode;

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

    /// One extra argument for rustfmt, e.g. `--rustfmt-arg=--config
    /// --rustfmt-arg=max_width=80`. Repeat for each argument.
    #[arg(long, allow_hyphen_values = true)]
    rustfmt_arg: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    rust_fmt_mf::formatter::set_extra_rustfmt_args(cli.rustfmt_arg.clone());
    let mut source = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut source) {
        eprintln!("rust-fmt-mf\tERROR\tcannot read stdin: {error}");
        return ExitCode::FAILURE;
    }
    // A panic is caught for the same reason an error is: either way the
    // source has to come back on stdout. The default hook still reports it.
    let outcome = std::panic::catch_unwind(|| format(&source, &cli))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("rust-fmt-mf panicked")));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // A filter that cannot format still has to hand back what it was
            // given. Vim's `formatprg` replaces the filtered range with this
            // process's stdout, so exiting with nothing written would delete
            // the user's code; the editor extension keys off the non-zero
            // status instead and falls back to plain rustfmt.
            eprintln!(
                "rust-fmt-mf\tERROR\t{}",
                format!("{error:#}").replace(['\r', '\n', '\t'], " ")
            );
            let _ = io::stdout().write_all(source.as_bytes());
            ExitCode::FAILURE
        }
    }
}

fn format(source: &str, cli: &Cli) -> anyhow::Result<()> {
    let result = rust_fmt_mf::format_source_with_report_and_options(
        source,
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
    // Written last, so any failure above leaves stdout untouched for `main`
    // to fill with the original source.
    io::stdout().write_all(result.text.as_bytes())?;
    Ok(())
}
