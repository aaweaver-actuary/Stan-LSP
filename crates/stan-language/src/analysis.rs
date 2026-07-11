//! Immutable analysis snapshots shared by editor features.

use std::sync::Arc;

use crate::{LexicalDiagnostic, Token, lex};

#[derive(Debug, Clone)]
pub struct Analysis {
    pub tokens: Arc<[Token]>,
    pub lexical_diagnostics: Arc<[LexicalDiagnostic]>,
}

/// Recomputes the complete local analysis for a document.
///
/// Future syntax and semantic stages can be added to this snapshot without
/// coupling them to the language-server transport.
pub fn analyze(text: &str) -> Analysis {
    let result = lex(text);
    Analysis {
        tokens: result.tokens.into(),
        lexical_diagnostics: result.diagnostics.into(),
    }
}
