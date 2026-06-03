use std::io::Write;
use std::process::{Command, Stdio};

/// Run rustfmt on the final result (without format_macro_bodies)
/// to format non-macro code and macro invocations.
pub fn run_rustfmt_no_macro(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let mut cmd = Command::new(rustfmt_path);
    cmd.args(["--edition", edition]);
    if let Some(path) = config_path {
        cmd.args(["--config-path", path]);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(source.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("rustfmt (final pass) failed: {}", stderr);
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Run rustfmt on the shadow file, returning the formatted result.
///
/// The shadow code is passed via stdin (spawn not exec), and the
/// formatted output is read from stdout.
pub fn run_rustfmt(
    shadow_code: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    let mut cmd = Command::new(rustfmt_path);
    cmd.args(["--edition", edition]);
    cmd.args(["--config", "format_macro_bodies=true"]);
    cmd.args(["--config", "format_macro_matchers=true"]);
    if let Some(path) = config_path {
        cmd.args(["--config-path", path]);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    // Write shadow code to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(shadow_code.as_bytes())?;
        // stdin is dropped here, closing the pipe
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("rustfmt failed: {}", stderr);
    }
    Ok(String::from_utf8(output.stdout)?)
}
