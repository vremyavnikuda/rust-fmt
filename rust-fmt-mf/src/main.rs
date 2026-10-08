use clap::Parser;
use std::io::{self, Read, Write};
use std::process::{Command, ExitCode, Stdio};

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

    /// Internal: format in this process instead of supervising a worker.
    #[arg(long, hide = true)]
    worker: bool,
}

/// The exit status a worker gives a failure it handled and reported itself.
const HANDLED_FAILURE: i32 = 1;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut source = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut source) {
        eprintln!("rust-fmt-mf\tERROR\tcannot read stdin: {error}");
        return ExitCode::FAILURE;
    }
    if cli.worker {
        rust_fmt_mf::formatter::set_extra_rustfmt_args(cli.rustfmt_arg.clone());
        return match format(&source, &cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                report_error(&format!("{error:#}"));
                ExitCode::from(HANDLED_FAILURE as u8)
            }
        };
    }
    supervise(&source)
}

/// Format in a child copy of this binary and hand back the source unless it
/// succeeded.
///
/// A filter that cannot format still has to return what it was given: Vim
/// and Neovim replace the filtered range with this process's stdout whatever
/// its exit status, so exiting with nothing written deletes the user's code.
/// An error can be handled in-process, but a stack overflow, an abort or an
/// out-of-memory kill ends the process before any handler runs. Only a
/// process that did not crash can still answer, so the work happens in
/// another one. The editor extension keys off the non-zero status and falls
/// back to plain rustfmt.
fn supervise(source: &str) -> ExitCode {
    let output = std::env::current_exe().and_then(|exe| {
        let mut child = Command::new(exe)
            .args(std::env::args_os().skip(1))
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        // The worker reads all of stdin before it writes anything, so this
        // cannot deadlock against a full stdout pipe.
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(source.as_bytes())?;
        }
        child.wait_with_output()
    });
    match output {
        Ok(output) if output.status.success() => {
            if io::stdout().write_all(&output.stdout).is_ok() {
                return ExitCode::SUCCESS;
            }
        }
        Ok(output) if output.status.code() == Some(HANDLED_FAILURE) => {}
        Ok(output) => report_error(&format!("formatter crashed: {}", output.status)),
        Err(error) => report_error(&format!("cannot run the formatter: {error}")),
    }
    let _ = io::stdout().write_all(source.as_bytes());
    ExitCode::FAILURE
}

fn report_error(message: &str) {
    eprintln!(
        "rust-fmt-mf\tERROR\t{}",
        message.replace(['\r', '\n', '\t'], " ")
    );
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
    // Written last: the supervisor discards a failed worker's stdout anyway,
    // but nothing half-written should ever be there to discard.
    io::stdout().write_all(result.text.as_bytes())?;
    Ok(())
}
