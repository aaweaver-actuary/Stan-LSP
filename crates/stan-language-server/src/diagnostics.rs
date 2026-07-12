//! Conversion and conservative merging of shared diagnostics into LSP values.

#![allow(
    missing_docs,
    reason = "adapter entry points mirror their explicit argument and return types"
)]

use stan_language::{Diagnostic as StanDiagnostic, DiagnosticSource, LintConfig, Severity, lint};
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location,
    NumberOrString, Range,
};

use crate::{document::Document, mapper::LspMapper};

pub fn diagnostics(document: &Document) -> Vec<Diagnostic> {
    diagnostics_with_lints(document, &LintConfig::default())
}

pub fn diagnostics_with_lints(document: &Document, lint_config: &LintConfig) -> Vec<Diagnostic> {
    document
        .analysis
        .diagnostics
        .iter()
        .cloned()
        .chain(lint(&document.analysis, lint_config))
        .filter_map(|diagnostic| to_lsp(document, diagnostic))
        .collect()
}

pub fn with_compiler(
    document: &Document,
    compiler: impl IntoIterator<Item = StanDiagnostic>,
) -> Vec<Diagnostic> {
    with_compiler_and_lints(document, compiler, &LintConfig::default())
}

pub fn with_compiler_and_lints(
    document: &Document,
    compiler: impl IntoIterator<Item = StanDiagnostic>,
    lint_config: &LintConfig,
) -> Vec<Diagnostic> {
    let mut merged = diagnostics_with_lints(document, lint_config);
    for diagnostic in compiler {
        let Some(diagnostic) = to_lsp(document, diagnostic) else {
            continue;
        };
        let duplicate = merged.iter().any(|existing| {
            existing.range == diagnostic.range && existing.message == diagnostic.message
        });
        if !duplicate {
            merged.push(diagnostic);
        }
    }
    merged
}

pub fn to_lsp(document: &Document, diagnostic: StanDiagnostic) -> Option<Diagnostic> {
    let start = diagnostic_position(document, diagnostic.primary_range.start, diagnostic.code.0)?;
    let end = diagnostic_position(document, diagnostic.primary_range.end, diagnostic.code.0)?;
    let related_information = diagnostic
        .related
        .iter()
        .filter_map(|related| {
            let start = diagnostic_position(document, related.range.start, diagnostic.code.0)?;
            let end = diagnostic_position(document, related.range.end, diagnostic.code.0)?;
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
                DiagnosticSource::Workspace => "stan-workspace",
            }
            .to_owned(),
        ),
        message: diagnostic.message,
        ..Diagnostic::default()
    })
}

fn diagnostic_position(
    document: &Document,
    offset: u32,
    code: &str,
) -> Option<tower_lsp_server::ls_types::Position> {
    match LspMapper::for_document(document).to_position(offset) {
        Ok(position) => Some(position),
        Err(error) => {
            tracing::error!(
                uri = %document.uri.as_str(),
                version = document.version,
                offset,
                code,
                ?error,
                "internal diagnostic range invariant failed"
            );
            None
        }
    }
}
