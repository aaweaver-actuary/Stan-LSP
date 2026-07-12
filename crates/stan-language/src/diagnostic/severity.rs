#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Editor-independent diagnostic severity.
#[allow(
    missing_docs,
    reason = "variants correspond directly to standard diagnostic severities"
)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}
