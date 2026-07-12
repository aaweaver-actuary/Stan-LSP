use crate::{ProgramBlockKind, TextRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Stable identity of a lexical scope within one semantic snapshot.
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Semantic purpose of a lexical scope.
#[allow(
    missing_docs,
    reason = "variants define the complete scope classification"
)]
pub enum ScopeKind {
    Root,
    ProgramBlock(ProgramBlockKind),
    Function,
    Loop,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One nested lexical scope.
pub struct Scope {
    /// Snapshot-local identity.
    pub id: ScopeId,
    /// Enclosing lexical scope, absent only for the root.
    pub parent: Option<ScopeId>,
    /// Source range governed by this scope.
    pub range: TextRange,
    /// Scope purpose used by visibility rules.
    pub kind: ScopeKind,
}
