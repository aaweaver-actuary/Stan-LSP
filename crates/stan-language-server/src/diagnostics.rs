use stan_language::{LexicalDiagnostic, analyze};
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};

use crate::document::Document;

pub fn diagnostics(document: &Document) -> Vec<Diagnostic> {
    analyze(&document.text)
        .lexical_diagnostics
        .iter()
        .copied()
        .filter_map(|diagnostic| to_lsp(document, diagnostic))
        .collect()
}

fn to_lsp(document: &Document, diagnostic: LexicalDiagnostic) -> Option<Diagnostic> {
    let start = document
        .line_index
        .offset_to_position(&document.text, diagnostic.range.start as usize)
        .ok()?;
    let end = document
        .line_index
        .offset_to_position(&document.text, diagnostic.range.end as usize)
        .ok()?;
    Some(Diagnostic {
        range: Range::new(start, end),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diagnostic.kind.code().to_owned())),
        source: Some("stan-lsp".to_owned()),
        message: diagnostic.kind.message().to_owned(),
        ..Diagnostic::default()
    })
}
