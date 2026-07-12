//! Diagnostics shared by the language server, formatter, and lint runner.

mod severity;

use crate::TextRange;

pub use severity::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Stable public identifier for a local, lint, workspace, or compiler finding.
pub struct DiagnosticCode(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Subsystem that produced a diagnostic.
#[allow(missing_docs, reason = "variants are the documented subsystem names")]
pub enum DiagnosticSource {
    StanLsp,
    Stanc3,
    StanLint,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Additional source location that explains a primary diagnostic.
pub struct RelatedDiagnostic {
    /// Related UTF-8 byte range in the same source file.
    pub range: TextRange,
    /// Explanation of its relationship to the primary finding.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Confidence that applying a suggested fix preserves source meaning.
#[allow(
    missing_docs,
    reason = "variants define the complete applicability scale"
)]
pub enum Applicability {
    Always,
    MaybeIncorrect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Replacement of one UTF-8 byte range.
pub struct TextEdit {
    /// Range to replace.
    pub range: TextRange,
    /// Replacement source text.
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// User-facing correction composed of one or more atomic edits.
pub struct Fix {
    /// Short action label suitable for an editor menu.
    pub label: String,
    /// Edits that must be applied atomically.
    pub edits: Vec<TextEdit>,
    /// Confidence classification for automatic application.
    pub applicability: Applicability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Shared transport-neutral finding consumed by LSP and `stanlint`.
pub struct Diagnostic {
    /// Stable diagnostic or lint identifier.
    pub code: DiagnosticCode,
    /// Configured severity.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
    /// Primary UTF-8 byte range.
    pub primary_range: TextRange,
    /// Supporting locations in the same source file.
    pub related: Vec<RelatedDiagnostic>,
    /// Optional fixes ordered by preference.
    pub fixes: Vec<Fix>,
    /// Subsystem that owns the finding.
    pub source: DiagnosticSource,
}

impl Diagnostic {
    /// Creates a high-confidence local error without related information or fixes.
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
