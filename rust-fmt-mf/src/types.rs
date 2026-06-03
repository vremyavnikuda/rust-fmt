use std::collections::HashMap;

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
}

impl Mapping {
    pub fn new() -> Self {
        Mapping {
            vars: HashMap::new(),
            next_id: 0,
        }
    }

    /// Register a macro variable and return its placeholder
    /// e.g., register("$x") → "__m_0"
    pub fn register(&mut self, original: &str) -> String {
        let placeholder = format!("__m_{}", self.next_id);
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
