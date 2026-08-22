use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static RUSTFMT_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Number of times `run_rustfmt`/`run_rustfmt_no_macro` have successfully
/// spawned a `rustfmt` process since the last `reset_rustfmt_call_count()`.
/// Test-only instrumentation for asserting on subprocess-spawn counts
/// instead of flaky wall-clock timing.
pub fn rustfmt_call_count() -> usize {
    RUSTFMT_CALL_COUNT.load(Ordering::SeqCst)
}

pub fn reset_rustfmt_call_count() {
    RUSTFMT_CALL_COUNT.store(0, Ordering::SeqCst);
}

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
    cmd.args(["--config", "format_macro_bodies=false"]);
    cmd.args(["--config", "format_macro_matchers=false"]);
    if let Some(path) = config_path {
        cmd.args(["--config-path", path]);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    RUSTFMT_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
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
    RUSTFMT_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
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
