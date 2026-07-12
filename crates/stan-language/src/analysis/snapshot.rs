use super::Revision;
use crate::{Diagnostic, LexicalDiagnostic, SemanticModel, StanType, SyntaxTree, TextRange, Token};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
/// Immutable result shared by all language and editor features for one revision.
pub struct AnalysisSnapshot {
    /// Revision of the source used to produce this snapshot.
    pub revision: Revision,
    /// Complete lossless token stream, including trivia and EOF.
    pub tokens: Arc<[Token]>,
    /// Recovery-oriented immutable syntax tree.
    pub syntax: SyntaxTree,
    /// Conservative local symbols, scopes, and references.
    pub semantics: SemanticModel,
    /// Lexer-native diagnostics retained for lexical consumers.
    pub lexical_diagnostics: Arc<[LexicalDiagnostic]>,
    /// Transport-neutral lexical, syntax, and verified semantic diagnostics.
    pub diagnostics: Arc<[Diagnostic]>,
    /// Types known with sufficient local confidence, keyed by source range.
    pub inferred_types: Arc<BTreeMap<TextRange, StanType>>,
    /// Include path spellings found in this file; resolution belongs to the workspace.
    pub include_dependencies: Arc<[String]>,
}
