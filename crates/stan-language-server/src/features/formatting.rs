//! Native full-document and syntax-aware range formatting adapters.

use tower_lsp_server::ls_types::{Range, TextEdit};

use crate::{document::Document, mapper::LspMapper};

pub fn formatting(document: &Document) -> Option<Vec<TextEdit>> {
    formatting_with_config(document, &stan_language::FormatterConfig::default())
}

pub fn formatting_with_config(
    document: &Document,
    config: &stan_language::FormatterConfig,
) -> Option<Vec<TextEdit>> {
    let formatted = stan_language::format(&document.analysis, config).ok()?;
    if formatted == document.text {
        return Some(Vec::new());
    }
    let range = LspMapper::for_document(document)
        .to_range(stan_language::TextRange::new(0, document.text.len()))
        .ok()?;
    Some(vec![TextEdit::new(range, formatted)])
}

pub fn range_formatting(document: &Document, requested: Range) -> Option<Vec<TextEdit>> {
    range_formatting_with_config(
        document,
        requested,
        &stan_language::FormatterConfig::default(),
    )
}

pub fn range_formatting_with_config(
    document: &Document,
    requested: Range,
    config: &stan_language::FormatterConfig,
) -> Option<Vec<TextEdit>> {
    let mapper = LspMapper::for_document(document);
    let start = mapper.to_offset(requested.start).ok()?;
    let end = mapper.to_offset(requested.end).ok()?;
    let edit = stan_language::format_range(
        &document.analysis,
        config,
        stan_language::TextRange { start, end },
    )
    .ok()?;
    Some(vec![TextEdit::new(
        mapper.to_range(edit.range).ok()?,
        edit.replacement,
    )])
}
