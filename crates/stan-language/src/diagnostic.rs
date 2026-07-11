//! Diagnostics shared by the language server, formatter, and lint runner.

use crate::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticCode(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSource {
    StanLsp,
    Stanc3,
    StanLint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedDiagnostic {
    pub range: TextRange,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Applicability {
    Always,
    MaybeIncorrect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: TextRange,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub label: String,
    pub edits: Vec<TextEdit>,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub primary_range: TextRange,
    pub related: Vec<RelatedDiagnostic>,
    pub fixes: Vec<Fix>,
    pub source: DiagnosticSource,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, range: TextRange) -> Self {
        Self {
            code: DiagnosticCode(code),
            severity: Severity::Error,
            message: message.into(),
            primary_range: range,
            related: Vec::new(),
            fixes: Vec::new(),
            source: DiagnosticSource::StanLsp,
        }
    }
}
