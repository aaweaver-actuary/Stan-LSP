//! Recomputes local analysis while attaching an editor-supplied revision.
//!
//! This module is used by the `AnalysisHost` to produce a new `AnalysisSnapshot` for a file when
//! its source text changes.
//!
//! It is also used by the formatter to produce a snapshot for formatting without mutating the host.

use std::sync::Arc;

use super::{AnalysisSnapshot, Revision};
use crate::{Diagnostic, SyntaxKind, Token, analyze_semantics, lex, parse};

/// Recomputes local analysis while attaching an editor-supplied revision.
pub fn analyze_revision(text: &str, revision: Revision) -> AnalysisSnapshot {
    let lexxed_text = lex(text);
    let tokens: Arc<[Token]> = lexxed_text.tokens.into();
    let parsed = parse(Arc::<str>::from(text), tokens.clone());
    let semantics = analyze_semantics(text, &tokens, &parsed.tree);
    let include_dependencies = tokens
        .iter()
        .filter(|token| token.kind == SyntaxKind::IncludePath)
        .map(|token| {
            text[token.range.start as usize..token.range.end as usize]
                .trim_matches('"')
                .to_owned()
        })
        .collect::<Vec<_>>();
    let diagnostics = lexxed_text
        .diagnostics
        .iter()
        .map(|diagnostic| {
            Diagnostic::error(
                diagnostic.kind.code(),
                diagnostic.kind.message(),
                diagnostic.range,
            )
        })
        .chain(parsed.diagnostics)
        .chain(semantics.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    let inferred_types = Arc::new(semantics.inferred_types.clone());
    AnalysisSnapshot {
        revision,
        tokens,
        syntax: parsed.tree,
        semantics,
        lexical_diagnostics: lexxed_text.diagnostics.into(),
        diagnostics: diagnostics.into(),
        inferred_types,
        include_dependencies: include_dependencies.into(),
    }
}
