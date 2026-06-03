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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    let formatted = rust_fmt_mf::format_source(
        &source,
        &cli.rustfmt_path,
        &cli.edition,
        cli.config_path.as_deref(),
    )?;
    io::stdout().write_all(formatted.as_bytes())?;
    Ok(())
}
