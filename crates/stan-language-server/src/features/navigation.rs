//! Scope-safe local definition, reference, highlight, and rename operations.

use std::collections::HashMap;

use tower_lsp_server::ls_types::{
    DocumentHighlight, DocumentHighlightKind, GotoDefinitionResponse, Location, TextEdit,
    WorkspaceEdit,
};

use crate::{document::Document, mapper::LspMapper};

pub fn goto_definition(document: &Document, offset: usize) -> Option<GotoDefinitionResponse> {
    let symbol = symbol_at(document, offset)?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        document.uri.clone(),
        LspMapper::for_document(document)
            .to_range(symbol.name_range)
            .ok()?,
    )))
}

pub fn references(
    document: &Document,
    offset: usize,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let symbol = symbol_at(document, offset)?;
    let mapper = LspMapper::for_document(document);
    let mut locations = Vec::new();
    if include_declaration {
        locations.push(Location::new(
            document.uri.clone(),
            mapper.to_range(symbol.name_range).ok()?,
        ));
    }
    locations.extend(
        document
            .analysis
            .semantics
            .references_to(symbol.id)
            .filter_map(|reference| {
                Some(Location::new(
                    document.uri.clone(),
                    mapper.to_range(reference.range).ok()?,
                ))
            }),
    );
    Some(locations)
}

pub fn highlights(document: &Document, offset: usize) -> Option<Vec<DocumentHighlight>> {
    Some(
        references(document, offset, true)?
            .into_iter()
            .map(|location| DocumentHighlight {
                range: location.range,
                kind: Some(DocumentHighlightKind::TEXT),
            })
            .collect(),
    )
}

pub fn rename(document: &Document, offset: usize, new_name: &str) -> Option<WorkspaceEdit> {
    if !valid_identifier(new_name) {
        return None;
    }
    let symbol = symbol_at(document, offset)?;
    if document.analysis.semantics.symbols.iter().any(|candidate| {
        candidate.id != symbol.id && candidate.scope == symbol.scope && candidate.name == new_name
    }) {
        return None;
    }
    let edits = references(document, offset, true)?
        .into_iter()
        .map(|location| TextEdit::new(location.range, new_name.to_owned()))
        .collect();
    Some(WorkspaceEdit {
        changes: Some(HashMap::from([(document.uri.clone(), edits)])),
        document_changes: None,
        change_annotations: None,
    })
}

pub(super) fn symbol_at(
    document: &Document,
    offset: usize,
) -> Option<&stan_language::SemanticSymbol> {
    document.analysis.semantics.symbol_at(offset as u32)
}

fn valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
        && stan_language::Keyword::from_str(name).is_err()
}
