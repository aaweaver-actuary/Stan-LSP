use stan_language::{Diagnostic as StanDiagnostic, DiagnosticSource, LintConfig, Severity, lint};
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location,
    NumberOrString, Range,
};

use crate::document::Document;

pub fn diagnostics(document: &Document) -> Vec<Diagnostic> {
    document
        .analysis
        .diagnostics
        .iter()
        .cloned()
        .chain(lint(&document.analysis, &LintConfig::default()))
        .filter_map(|diagnostic| to_lsp(document, diagnostic))
        .collect()
}

pub fn to_lsp(document: &Document, diagnostic: StanDiagnostic) -> Option<Diagnostic> {
    let start = document
        .line_index
        .offset_to_position(&document.text, diagnostic.primary_range.start as usize)
        .ok()?;
    let end = document
        .line_index
        .offset_to_position(&document.text, diagnostic.primary_range.end as usize)
        .ok()?;
    let related_information = diagnostic
        .related
        .iter()
        .filter_map(|related| {
            let start = document
                .line_index
                .offset_to_position(&document.text, related.range.start as usize)
                .ok()?;
            let end = document
                .line_index
                .offset_to_position(&document.text, related.range.end as usize)
                .ok()?;
            Some(DiagnosticRelatedInformation {
                location: Location::new(document.uri.clone(), Range::new(start, end)),
                message: related.message.clone(),
            })
        })
        .collect::<Vec<_>>();
    Some(Diagnostic {
        range: Range::new(start, end),
        severity: Some(match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Information => DiagnosticSeverity::INFORMATION,
            Severity::Hint => DiagnosticSeverity::HINT,
        }),
        code: Some(NumberOrString::String(diagnostic.code.0.to_owned())),
        related_information: (!related_information.is_empty()).then_some(related_information),
        tags: diagnostic
            .code
            .0
            .starts_with("deprecated.")
            .then_some(vec![DiagnosticTag::DEPRECATED]),
        source: Some(
            match diagnostic.source {
                DiagnosticSource::StanLsp => "stan-lsp",
                DiagnosticSource::Stanc3 => "stanc3",
                DiagnosticSource::StanLint => "stanlint",
            }
            .to_owned(),
        ),
        message: diagnostic.message,
        ..Diagnostic::default()
    })
}
