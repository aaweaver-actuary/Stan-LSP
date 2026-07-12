use crate::TextRange;
use crate::semantics::{scope, symbol};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Stable identity of an identifier reference within one semantic snapshot.
pub struct ReferenceId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
/// One identifier use and its optional conservative resolution.
pub struct Reference {
    /// Snapshot-local identity.
    pub id: ReferenceId,
    /// Identifier spelling.
    pub name: String,
    /// Exact use range.
    pub range: TextRange,
    /// Innermost lexical scope at the use.
    pub scope: scope::ScopeId,
    /// Resolved declaration, absent when unsupported or genuinely unresolved.
    pub resolved: Option<symbol::SymbolId>,
}
