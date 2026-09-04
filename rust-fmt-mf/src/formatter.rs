use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

static RUSTFMT_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Number of times `run_rustfmt`/`run_rustfmt_no_macro` have successfully
/// spawned a `rustfmt` process since the last `reset_rustfmt_call_count()`.
/// Instrumentation for asserting on subprocess-spawn counts in tests
/// instead of flaky wall-clock timing. Cache hits do not count: the point
/// of the counter is real process spawns.
pub fn rustfmt_call_count() -> usize {
    RUSTFMT_CALL_COUNT.load(Ordering::SeqCst)
}

pub fn reset_rustfmt_call_count() {
    RUSTFMT_CALL_COUNT.store(0, Ordering::SeqCst);
    cache().lock().unwrap().clear();
}

/// (format_macros, rustfmt_path, edition, config_path, source)
type CacheKey = (bool, String, String, Option<String>, String);

/// ponytail: unbounded process-local memo. `rust-fmt-mf` is one-shot per
/// file, and the convergence loop feeds the same text to rustfmt over and
/// over, so entries are bounded by that file's distinct inputs. Add an LRU
/// only if this ever becomes a long-lived server.
fn cache() -> &'static Mutex<HashMap<CacheKey, Result<String, String>>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, Result<String, String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Run rustfmt on the final result (without format_macro_bodies)
/// to format non-macro code and macro invocations.
pub fn run_rustfmt_no_macro(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
) -> anyhow::Result<String> {
    run_cached(source, rustfmt_path, edition, config_path, false)
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
    run_cached(shadow_code, rustfmt_path, edition, config_path, true)
}

/// rustfmt is deterministic for a given (binary, args, stdin), so both
/// successes and failures are memoized: the convergence loop and the
/// `try_format_as_mod().or_else(try_format_as_fn())` pairs re-ask the same
/// questions many times per file.
fn run_cached(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    format_macros: bool,
) -> anyhow::Result<String> {
    let key: CacheKey = (
        format_macros,
        rustfmt_path.to_string(),
        edition.to_string(),
        config_path.map(str::to_string),
        source.to_string(),
    );
    if let Some(hit) = cache().lock().unwrap().get(&key) {
        return hit.clone().map_err(|message| anyhow::anyhow!(message));
    }
    let result = spawn_rustfmt(source, rustfmt_path, edition, config_path, format_macros);
    // Only outcomes rustfmt itself produced are cacheable; a spawn/IO error
    // (missing binary, broken pipe) is an environment fault that may not
    // repeat, so it is neither stored nor allowed to poison later calls.
    let cacheable = match &result {
        Ok(text) => Some(Ok(text.clone())),
        Err(RustfmtError::Rejected(message)) => Some(Err(message.clone())),
        Err(RustfmtError::Spawn(_)) => None,
    };
    if let Some(entry) = cacheable {
        cache().lock().unwrap().insert(key, entry);
    }
    match result {
        Ok(text) => Ok(text),
        Err(RustfmtError::Rejected(message)) => Err(anyhow::anyhow!(message)),
        Err(RustfmtError::Spawn(error)) => Err(error),
    }
}

enum RustfmtError {
    /// rustfmt ran and refused the input (non-zero exit, or non-UTF-8 output).
    Rejected(String),
    /// rustfmt could not be run at all.
    Spawn(anyhow::Error),
}

fn spawn_rustfmt(
    source: &str,
    rustfmt_path: &str,
    edition: &str,
    config_path: Option<&str>,
    format_macros: bool,
) -> Result<String, RustfmtError> {
    let toggle = if format_macros { "true" } else { "false" };
    let mut cmd = Command::new(rustfmt_path);
    cmd.args(["--edition", edition]);
    cmd.args(["--config", &format!("format_macro_bodies={toggle}")]);
    cmd.args(["--config", &format!("format_macro_matchers={toggle}")]);
    if let Some(path) = config_path {
        cmd.args(["--config-path", path]);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|error| RustfmtError::Spawn(error.into()))?;
    RUSTFMT_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(source.as_bytes())
            .map_err(|error| RustfmtError::Spawn(error.into()))?;
        // stdin is dropped here, closing the pipe
    }
    let output = child
        .wait_with_output()
        .map_err(|error| RustfmtError::Spawn(error.into()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let label = if format_macros {
            "rustfmt failed"
        } else {
            "rustfmt (final pass) failed"
        };
        return Err(RustfmtError::Rejected(format!("{label}: {stderr}")));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        RustfmtError::Rejected(format!("rustfmt emitted non-UTF-8 output: {error}"))
    })
}
