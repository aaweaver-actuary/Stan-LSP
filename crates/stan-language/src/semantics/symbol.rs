use crate::{StanType, TextRange, TextSize, semantics::scope::ScopeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Stable identity of a declaration within one semantic snapshot.
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Domain role of a declaration.
#[allow(
    missing_docs,
    reason = "variants are the documented Stan declaration roles"
)]
pub enum SemanticSymbolKind {
    Data,
    Parameter,
    TransformedData,
    TransformedParameter,
    GeneratedQuantity,
    LocalVariable,
    LoopVariable,
    Function,
    FunctionParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One declaration and the visibility facts needed for conservative resolution.
pub struct SemanticSymbol {
    /// Snapshot-local identity.
    pub id: SymbolId,
    /// Declared spelling.
    pub name: String,
    /// Stan declaration role.
    pub kind: SemanticSymbolKind,
    /// Complete declaration range.
    pub declaration: TextRange,
    /// Exact name range.
    pub name_range: TextRange,
    /// Locally known declared type, if representable.
    pub declared_type: Option<StanType>,
    /// Scope that owns this declaration.
    pub scope: ScopeId,
    /// Earliest byte offset at which this declaration is visible.
    pub visible_from: TextSize,
}
