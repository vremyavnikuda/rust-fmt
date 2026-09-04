use std::collections::HashMap;
use std::ops::Range;

/// Formatting knobs that are not part of `rustfmt`'s own configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatOptions {
    /// Delete blank lines inside braces, so a large file does not grow by
    /// hundreds of lines of vertical whitespace. Off by default: `rustfmt`
    /// keeps the author's blank lines, and silently deleting them would
    /// make every first format produce a diff that has nothing to do with
    /// macros. The rule only ever removes; the blank lines this crate
    /// *inserts* between items are not affected.
    pub compact_blank_lines: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroStatus {
    Formatted,
    Unchanged,
    Skipped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroOutcome {
    pub name: String,
    pub span: Range<usize>,
    pub status: MacroStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub text: String,
    pub macros: Vec<MacroOutcome>,
}

/// Represents a single macro_rules! definition found in source
#[derive(Debug, Clone)]
pub struct MacroDef {
    /// Name of the macro (e.g., "field_accessor")
    pub name: String,
    /// Byte span in the original source (start..end)
    pub span: std::ops::Range<usize>,
    /// Individual arms: pattern => { body }
    pub arms: Vec<MacroArm>,
}

/// One arm of a macro_rules! definition
#[derive(Debug, Clone)]
pub struct MacroArm {
    /// Byte span of the pattern part (before => {)
    pub pattern_span: std::ops::Range<usize>,
    /// Byte span of the body (inside { ... } after =>)
    pub body_span: std::ops::Range<usize>,
}

/// Mapping from placeholder identifier to original macro text
#[derive(Debug, Clone)]
pub struct Mapping {
    /// $var → __m_N mapping: placeholder → original_text
    pub vars: HashMap<String, String>,
    /// Counter for unique placeholder names
    next_id: usize,
    placeholder_prefix: String,
    marker_prefix: String,
}

impl Mapping {
    pub fn new() -> Self {
        Mapping {
            vars: HashMap::new(),
            next_id: 0,
            placeholder_prefix: "__m_".to_string(),
            marker_prefix: "__mf_".to_string(),
        }
    }

    pub fn with_prefix(prefix: String) -> Self {
        Mapping {
            vars: HashMap::new(),
            next_id: 0,
            placeholder_prefix: format!("{prefix}var_"),
            marker_prefix: prefix,
        }
    }

    pub fn marker_prefix(&self) -> &str {
        &self.marker_prefix
    }

    pub fn repetition_marker(&self, kind: &str) -> String {
        format!("{}rep_{kind}", self.marker_prefix)
    }

    /// Register a macro variable and return its placeholder
    /// e.g., register("$x") → "__m_0"
    pub fn register(&mut self, original: &str) -> String {
        let placeholder = format!("{}{}", self.placeholder_prefix, self.next_id);
        self.next_id += 1;
        self.vars.insert(placeholder.clone(), original.to_string());
        placeholder
    }

    /// Restore original macro text from placeholder
    /// e.g., restore("__m_0") → "$x"
    pub fn restore(&self, placeholder: &str) -> Option<&String> {
        self.vars.get(placeholder)
    }
}

impl Default for Mapping {
    fn default() -> Self {
        Self::new()
    }
}
