use std::collections::HashMap;

use stan_language::{
    Distribution, FunctionCategory, FunctionSignature, KeywordRole, Lifecycle, StanFunction,
    SyntaxKind,
};
use tower_lsp_server::ls_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, CodeAction,
    CodeActionKind, CodeActionOrCommand, CodeActionResponse, CompletionItem, CompletionItemKind,
    CompletionResponse, DocumentHighlight, DocumentHighlightKind, DocumentSymbol,
    DocumentSymbolResponse, FoldingRange, FoldingRangeKind, GotoDefinitionResponse, Hover,
    HoverContents, InlayHint, InlayHintKind, InlayHintLabel, Location, MarkupContent, MarkupKind,
    Range, SemanticToken, SemanticTokens, SemanticTokensResult, SignatureHelp,
    SignatureInformation, SymbolKind, TextEdit, WorkspaceEdit,
};

use crate::document::Document;

pub fn document_symbols(document: &Document) -> DocumentSymbolResponse {
    let mut symbols = document
        .analysis
        .syntax
        .source_file()
        .program_blocks()
        .filter_map(|block| {
            let range = to_range(document, block.range())?;
            #[allow(deprecated)]
            Some(DocumentSymbol {
                name: block.kind().as_str().to_owned(),
                detail: Some("Stan program block".to_owned()),
                kind: SymbolKind::NAMESPACE,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            })
        })
        .collect::<Vec<_>>();
    symbols.extend(
        document
            .analysis
            .semantics
            .symbols
            .iter()
            .filter_map(|symbol| {
                let range = to_range(document, symbol.declaration)?;
                let selection_range = to_range(document, symbol.name_range)?;
                #[allow(deprecated)]
                Some(DocumentSymbol {
                    name: symbol.name.clone(),
                    detail: symbol
                        .declared_type
                        .as_ref()
                        .map(|kind| format!("{kind:?}")),
                    kind: match symbol.kind {
                        stan_language::SemanticSymbolKind::Function => SymbolKind::FUNCTION,
                        _ => SymbolKind::VARIABLE,
                    },
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: None,
                })
            }),
    );
    DocumentSymbolResponse::Nested(symbols)
}

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
    let range = to_range(
        document,
        stan_language::TextRange::new(0, document.text.len()),
    )?;
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
    let start = document
        .line_index
        .position_to_offset(&document.text, requested.start)
        .ok()?;
    let end = document
        .line_index
        .position_to_offset(&document.text, requested.end)
        .ok()?;
    let edit = stan_language::format_range(
        &document.analysis,
        config,
        stan_language::TextRange::new(start, end),
    )
    .ok()?;
    Some(vec![TextEdit::new(
        to_range(document, edit.range)?,
        edit.replacement,
    )])
}

pub fn completion(document: &Document, offset: usize) -> CompletionResponse {
    let distribution_context = document
        .analysis
        .tokens
        .iter()
        .rev()
        .find(|token| {
            token.range.end as usize <= offset
                && !matches!(
                    token.kind,
                    SyntaxKind::Whitespace
                        | SyntaxKind::LineComment
                        | SyntaxKind::BlockComment
                        | SyntaxKind::EndOfFile
                )
        })
        .is_some_and(|token| token.kind == SyntaxKind::Symbol(stan_language::Symbol::Tilde));
    let mut items: Vec<CompletionItem> = if distribution_context {
        Distribution::ALL
            .iter()
            .map(|distribution| CompletionItem {
                label: distribution.as_str().to_owned(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("{:?} distribution", distribution.kind())),
                ..CompletionItem::default()
            })
            .collect()
    } else {
        StanFunction::ALL
            .iter()
            .filter(|function| {
                function
                    .metadata()
                    .lifecycle
                    .is_available_in(stan_language::STAN_VERSION)
            })
            .map(|function| CompletionItem {
                label: function.as_str().to_owned(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("{} overloads", function.signatures().len())),
                sort_text: Some(format!("1-{}", function.as_str())),
                ..CompletionItem::default()
            })
            .collect()
    };
    if !distribution_context {
        items.extend(document.analysis.semantics.symbols.iter().map(|symbol| {
            CompletionItem {
                label: symbol.name.clone(),
                kind: Some(match symbol.kind {
                    stan_language::SemanticSymbolKind::Function => CompletionItemKind::FUNCTION,
                    _ => CompletionItemKind::VARIABLE,
                }),
                detail: symbol
                    .declared_type
                    .as_ref()
                    .map(|kind| format!("{kind:?}")),
                sort_text: Some(format!("0-{}", symbol.name)),
                ..CompletionItem::default()
            }
        }));
    }
    CompletionResponse::Array(items)
}

pub fn goto_definition(document: &Document, offset: usize) -> Option<GotoDefinitionResponse> {
    let symbol = symbol_at(document, offset)?;
    Some(GotoDefinitionResponse::Scalar(Location::new(
        document.uri.clone(),
        to_range(document, symbol.name_range)?,
    )))
}

pub fn references(
    document: &Document,
    offset: usize,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let symbol = symbol_at(document, offset)?;
    let mut locations = Vec::new();
    if include_declaration {
        locations.push(Location::new(
            document.uri.clone(),
            to_range(document, symbol.name_range)?,
        ));
    }
    locations.extend(
        document
            .analysis
            .semantics
            .references
            .iter()
            .filter(|reference| reference.resolved == Some(symbol.id))
            .filter_map(|reference| {
                Some(Location::new(
                    document.uri.clone(),
                    to_range(document, reference.range)?,
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

pub fn code_actions(document: &Document, requested: Range) -> CodeActionResponse {
    code_actions_with_config(document, requested, &stan_language::LintConfig::default())
}

pub fn code_actions_with_config(
    document: &Document,
    requested: Range,
    config: &stan_language::LintConfig,
) -> CodeActionResponse {
    document
        .analysis
        .diagnostics
        .iter()
        .cloned()
        .chain(stan_language::lint(&document.analysis, config))
        .filter(|diagnostic| {
            to_range(document, diagnostic.primary_range)
                .is_some_and(|range| overlaps(range, requested))
        })
        .flat_map(|diagnostic| diagnostic.fixes)
        .filter_map(|fix| {
            let edits = fix
                .edits
                .into_iter()
                .map(|edit| {
                    Some(TextEdit::new(
                        to_range(document, edit.range)?,
                        edit.replacement,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(CodeActionOrCommand::CodeAction(CodeAction {
                title: fix.label,
                kind: Some(CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    changes: Some(HashMap::from([(document.uri.clone(), edits)])),
                    document_changes: None,
                    change_annotations: None,
                }),
                is_preferred: Some(fix.applicability == stan_language::Applicability::Always),
                ..CodeAction::default()
            }))
        })
        .collect()
}

pub fn prepare_call_hierarchy(
    document: &Document,
    offset: usize,
) -> Option<Vec<CallHierarchyItem>> {
    let symbol = symbol_at(document, offset)?;
    (symbol.kind == stan_language::SemanticSymbolKind::Function)
        .then(|| hierarchy_item(document, symbol))
        .flatten()
        .map(|item| vec![item])
}

pub fn inlay_hints(document: &Document, requested: Range) -> Vec<InlayHint> {
    let text = document.analysis.syntax.text();
    let mut hints = Vec::new();
    for node in document
        .analysis
        .syntax
        .nodes()
        .iter()
        .filter(|node| node.kind == stan_language::SyntaxNodeKind::FunctionCall)
    {
        let Some(name_token) = document.analysis.tokens.get(node.token_range.start) else {
            continue;
        };
        let name = &text[name_token.range.start as usize..name_token.range.end as usize];
        let Ok(function) = StanFunction::from_str(name) else {
            continue;
        };
        let Some(parameters) = function
            .signatures()
            .iter()
            .find_map(|signature| match signature {
                FunctionSignature::Concrete(signature) => Some(&signature.parameters),
                FunctionSignature::Variadic(_) => None,
            })
        else {
            continue;
        };
        let starts = argument_starts(&document.analysis.tokens[node.token_range.clone()]);
        for (parameter, token) in parameters.iter().zip(starts) {
            if parameter.qualifier != stan_language::DataQualifier::DataOnly {
                continue;
            }
            let Some(position) = document
                .line_index
                .offset_to_position(&document.text, token.range.start as usize)
                .ok()
            else {
                continue;
            };
            if position < requested.start || position > requested.end {
                continue;
            }
            hints.push(InlayHint {
                position,
                label: InlayHintLabel::String("data: ".to_owned()),
                kind: Some(InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: Some(false),
                padding_right: Some(true),
                data: None,
            });
        }
    }
    hints
}

fn argument_starts(tokens: &[stan_language::Token]) -> Vec<stan_language::Token> {
    let mut starts = Vec::new();
    let mut depth = 0usize;
    let mut need_start = true;
    for token in tokens.iter().skip(1) {
        match token.kind {
            SyntaxKind::Symbol(stan_language::Symbol::LeftParen) => depth += 1,
            SyntaxKind::Symbol(stan_language::Symbol::RightParen) if depth == 1 => break,
            SyntaxKind::Symbol(stan_language::Symbol::RightParen) => {
                depth = depth.saturating_sub(1);
            }
            SyntaxKind::Symbol(stan_language::Symbol::Comma) if depth == 1 => need_start = true,
            SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment => {}
            _ if depth == 1 && need_start => {
                starts.push(*token);
                need_start = false;
            }
            _ => {}
        }
    }
    starts
}

pub fn incoming_calls(
    document: &Document,
    item: &CallHierarchyItem,
) -> Vec<CallHierarchyIncomingCall> {
    let Some(target) = hierarchy_symbol(document, item) else {
        return Vec::new();
    };
    let mut grouped = HashMap::<stan_language::SymbolId, Vec<Range>>::new();
    for reference in document
        .analysis
        .semantics
        .references
        .iter()
        .filter(|reference| reference.resolved == Some(target.id))
    {
        let caller = document
            .analysis
            .semantics
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == stan_language::SemanticSymbolKind::Function)
            .find(|symbol| {
                symbol.declaration.start <= reference.range.start
                    && reference.range.end <= symbol.declaration.end
            });
        if let (Some(caller), Some(range)) = (caller, to_range(document, reference.range)) {
            grouped.entry(caller.id).or_default().push(range);
        }
    }
    grouped
        .into_iter()
        .filter_map(|(id, from_ranges)| {
            let symbol = document
                .analysis
                .semantics
                .symbols
                .iter()
                .find(|symbol| symbol.id == id)?;
            Some(CallHierarchyIncomingCall {
                from: hierarchy_item(document, symbol)?,
                from_ranges,
            })
        })
        .collect()
}

pub fn outgoing_calls(
    document: &Document,
    item: &CallHierarchyItem,
) -> Vec<CallHierarchyOutgoingCall> {
    let Some(caller) = hierarchy_symbol(document, item) else {
        return Vec::new();
    };
    let mut grouped = HashMap::<stan_language::SymbolId, Vec<Range>>::new();
    for reference in &document.analysis.semantics.references {
        if !(caller.declaration.start <= reference.range.start
            && reference.range.end <= caller.declaration.end)
        {
            continue;
        }
        let Some(target) = reference.resolved.and_then(|id| {
            document
                .analysis
                .semantics
                .symbols
                .iter()
                .find(|symbol| symbol.id == id)
        }) else {
            continue;
        };
        if target.kind != stan_language::SemanticSymbolKind::Function {
            continue;
        }
        if let Some(range) = to_range(document, reference.range) {
            grouped.entry(target.id).or_default().push(range);
        }
    }
    grouped
        .into_iter()
        .filter_map(|(id, from_ranges)| {
            let symbol = document
                .analysis
                .semantics
                .symbols
                .iter()
                .find(|symbol| symbol.id == id)?;
            Some(CallHierarchyOutgoingCall {
                to: hierarchy_item(document, symbol)?,
                from_ranges,
            })
        })
        .collect()
}

fn hierarchy_symbol<'a>(
    document: &'a Document,
    item: &CallHierarchyItem,
) -> Option<&'a stan_language::SemanticSymbol> {
    let id = item.data.as_ref()?.as_u64()? as u32;
    document
        .analysis
        .semantics
        .symbols
        .iter()
        .find(|symbol| symbol.id.0 == id)
}

fn hierarchy_item(
    document: &Document,
    symbol: &stan_language::SemanticSymbol,
) -> Option<CallHierarchyItem> {
    Some(CallHierarchyItem {
        name: symbol.name.clone(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail: symbol
            .declared_type
            .as_ref()
            .map(|kind| format!("{kind:?}")),
        uri: document.uri.clone(),
        range: to_range(document, symbol.declaration)?,
        selection_range: to_range(document, symbol.name_range)?,
        data: Some(serde_json::json!(symbol.id.0)),
    })
}

fn overlaps(left: Range, right: Range) -> bool {
    left.start <= right.end && right.start <= left.end
}

fn symbol_at(document: &Document, offset: usize) -> Option<&stan_language::SemanticSymbol> {
    document
        .analysis
        .semantics
        .symbols
        .iter()
        .find(|symbol| contains(symbol.name_range, offset))
        .or_else(|| {
            let reference = document
                .analysis
                .semantics
                .references
                .iter()
                .find(|reference| contains(reference.range, offset))?;
            document
                .analysis
                .semantics
                .symbols
                .iter()
                .find(|symbol| Some(symbol.id) == reference.resolved)
        })
}

fn contains(range: stan_language::TextRange, offset: usize) -> bool {
    range.start as usize <= offset && offset < range.end as usize
}

fn valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
        && stan_language::Keyword::from_str(name).is_err()
}

pub fn signature_help(document: &Document, offset: usize) -> Option<SignatureHelp> {
    let before = &document.text[..offset];
    let open = before.rfind('(')?;
    let name_end = open;
    let name_start = before[..name_end]
        .rfind(|character: char| !(character == '_' || character.is_alphanumeric()))
        .map_or(0, |index| index + 1);
    let name = &before[name_start..name_end];
    let function = StanFunction::from_str(name).ok().or_else(|| {
        let distribution = Distribution::from_str(name).ok()?;
        let suffix = match distribution.kind() {
            stan_language::DistributionKind::Continuous => "lpdf",
            stan_language::DistributionKind::Discrete => "lpmf",
        };
        StanFunction::from_str(&format!("{name}_{suffix}")).ok()
    })?;
    let active_parameter = before[open + 1..]
        .chars()
        .filter(|character| *character == ',')
        .count() as u32;
    let signatures = function
        .signatures()
        .iter()
        .map(|signature| SignatureInformation {
            label: signature_label(function, signature),
            documentation: None,
            parameters: None,
            active_parameter: None,
        })
        .collect();
    Some(SignatureHelp {
        signatures,
        active_signature: Some(0),
        active_parameter: Some(active_parameter),
    })
}

fn signature_label(function: StanFunction, signature: &FunctionSignature) -> String {
    signature.display(function.as_str())
}

pub fn folding_ranges(document: &Document) -> Vec<FoldingRange> {
    let mut ranges = document
        .analysis
        .syntax
        .source_file()
        .program_blocks()
        .filter_map(|block| {
            let range = to_range(document, block.range())?;
            (range.start.line < range.end.line).then_some(FoldingRange {
                start_line: range.start.line,
                start_character: Some(range.start.character),
                end_line: range.end.line,
                end_character: Some(range.end.character),
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: Some(block.kind().as_str().to_owned()),
            })
        })
        .collect::<Vec<_>>();
    ranges.extend(document.analysis.tokens.iter().filter_map(|token| {
        if token.kind != SyntaxKind::BlockComment {
            return None;
        }
        let range = to_range(document, token.range)?;
        (range.start.line < range.end.line).then_some(FoldingRange {
            start_line: range.start.line,
            start_character: Some(range.start.character),
            end_line: range.end.line,
            end_character: Some(range.end.character),
            kind: Some(FoldingRangeKind::Comment),
            collapsed_text: None,
        })
    }));
    ranges
}

pub fn semantic_tokens(document: &Document) -> SemanticTokensResult {
    let mut encoded = Vec::new();
    let mut previous_line = 0;
    let mut previous_start = 0;
    for token in document.analysis.tokens.iter() {
        let Some((token_type, modifiers)) = semantic_kind(token.kind) else {
            continue;
        };
        let Some(range) = to_range(document, token.range) else {
            continue;
        };
        if range.start.line != range.end.line {
            continue;
        }
        let delta_line = range.start.line - previous_line;
        let delta_start = if delta_line == 0 {
            range.start.character - previous_start
        } else {
            range.start.character
        };
        encoded.push(SemanticToken {
            delta_line,
            delta_start,
            length: range.end.character - range.start.character,
            token_type,
            token_modifiers_bitset: modifiers,
        });
        previous_line = range.start.line;
        previous_start = range.start.character;
    }
    SemanticTokensResult::Tokens(SemanticTokens {
        result_id: Some(document.version.to_string()),
        data: encoded,
    })
}

pub fn hover(document: &Document, offset: usize) -> Option<Hover> {
    let token =
        document.analysis.tokens.iter().find(|token| {
            token.range.start as usize <= offset && offset < token.range.end as usize
        })?;
    if token.kind != SyntaxKind::Identifier {
        return None;
    }
    let name = &document.text[token.range.start as usize..token.range.end as usize];
    if let Some(symbol) = symbol_at(document, offset) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "```stan\n{}{}\n```\n\n{:?}",
                    symbol
                        .declared_type
                        .as_ref()
                        .map(|kind| format!("{kind:?} "))
                        .unwrap_or_default(),
                    symbol.name,
                    symbol.kind
                ),
            }),
            range: to_range(document, token.range),
        });
    }
    let Ok(function) = StanFunction::from_str(name) else {
        let distribution = Distribution::from_str(name).ok()?;
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "```stan\n{name}(...)\n```\n\n**{:?} distribution**\n\n[Stan functions reference](https://mc-stan.org/docs/functions-reference/)",
                    distribution.kind()
                ),
            }),
            range: to_range(document, token.range),
        });
    };
    let metadata = function.metadata();
    let categories = metadata
        .categories
        .iter()
        .map(category_name)
        .collect::<Vec<_>>()
        .join(", ");
    let lifecycle = match metadata.lifecycle {
        Lifecycle::Active { .. } => "active".to_owned(),
        Lifecycle::Deprecated { replacement, .. } => replacement.map_or_else(
            || "deprecated".to_owned(),
            |replacement| format!("deprecated; use `{replacement}`"),
        ),
        Lifecycle::Removed { replacement, .. } => replacement.map_or_else(
            || "removed".to_owned(),
            |replacement| format!("removed; use `{replacement}`"),
        ),
    };
    let value = format!(
        "```stan\n{name}(...)\n```\n\n**{} overloads** · {categories} · {lifecycle}\n\n[Stan functions reference](https://mc-stan.org/docs/functions-reference/)",
        function.signatures().len()
    );
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: to_range(document, token.range),
    })
}

fn category_name(category: &FunctionCategory) -> &'static str {
    match category {
        FunctionCategory::ScalarMath => "scalar math",
        FunctionCategory::ComplexMath => "complex math",
        FunctionCategory::Array => "array",
        FunctionCategory::Matrix => "matrix",
        FunctionCategory::ComplexMatrix => "complex matrix",
        FunctionCategory::SparseMatrix => "sparse matrix",
        FunctionCategory::Mixed => "mixed",
        FunctionCategory::HigherOrder => "higher order",
        FunctionCategory::Transform => "transform",
        FunctionCategory::Probability => "probability",
        FunctionCategory::HiddenMarkov => "hidden Markov",
        FunctionCategory::EmbeddedLaplace => "embedded Laplace",
        FunctionCategory::Utility => "utility",
    }
}

fn semantic_kind(kind: SyntaxKind) -> Option<(u32, u32)> {
    match kind {
        SyntaxKind::Keyword(keyword) if keyword.roles().contains(&KeywordRole::Type) => {
            Some((1, 0))
        }
        SyntaxKind::Keyword(_) => Some((0, 0)),
        SyntaxKind::Identifier => Some((3, 0)),
        SyntaxKind::Symbol(_) => Some((4, 0)),
        SyntaxKind::IntegerLiteral | SyntaxKind::RealLiteral | SyntaxKind::ImaginaryLiteral => {
            Some((5, 0))
        }
        SyntaxKind::StringLiteral => Some((6, 0)),
        SyntaxKind::LineComment | SyntaxKind::BlockComment => Some((7, 0)),
        SyntaxKind::Directive(_) => Some((8, 0)),
        SyntaxKind::IncludePath => Some((6, 0)),
        SyntaxKind::Legacy(_) => Some((0, 1)),
        _ => None,
    }
}

fn to_range(document: &Document, range: stan_language::TextRange) -> Option<Range> {
    Some(Range::new(
        document
            .line_index
            .offset_to_position(&document.text, range.start as usize)
            .ok()?,
        document
            .line_index
            .offset_to_position(&document.text, range.end as usize)
            .ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Uri;

    fn document(source: &str) -> Document {
        let uri: Uri = "file:///model.stan".parse().unwrap();
        Document::new(uri, 1, source.to_owned())
    }

    #[test]
    fn structural_and_catalog_features_share_one_snapshot() {
        let source = "parameters { real theta; } model { theta ~ normal(0, 1); }";
        let document = document(source);
        assert!(
            match document_symbols(&document) {
                DocumentSymbolResponse::Nested(symbols) => symbols.len(),
                _ => 0,
            } >= 2
        );
        assert!(hover(&document, source.find("normal").unwrap()).is_some());
        assert!(goto_definition(&document, source.rfind("theta").unwrap()).is_some());
        assert!(
            !references(&document, source.rfind("theta").unwrap(), true)
                .unwrap()
                .is_empty()
        );
        assert!(
            !semantic_tokens(&document).eq(&SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data: Vec::new(),
            }))
        );
    }

    #[test]
    fn completion_formatting_and_safe_fixes_are_available() {
        let distribution = document("model { real y; y ~ ");
        let CompletionResponse::Array(items) = completion(&distribution, distribution.text.len())
        else {
            panic!("expected completion array");
        };
        assert!(items.iter().any(|item| item.label == "normal"));

        let unformatted = document("model{real y;}");
        assert!(formatting(&unformatted).is_some());

        let legacy = document("model { real x; x <- 1; }");
        let actions = code_actions(
            &legacy,
            Range::new(
                tower_lsp_server::ls_types::Position::new(0, 0),
                tower_lsp_server::ls_types::Position::new(0, legacy.text.len() as u32),
            ),
        );
        assert!(actions.iter().any(|action| matches!(action, CodeActionOrCommand::CodeAction(action) if action.title.contains("replace"))));
    }

    #[test]
    fn call_hierarchy_uses_resolved_user_functions() {
        let source = "functions { real helper(real x) { return x; } real outer(real y) { return helper(y); } }";
        let document = document(source);
        let items = prepare_call_hierarchy(&document, source.find("helper").unwrap()).unwrap();
        let incoming = incoming_calls(&document, &items[0]);
        assert!(incoming.iter().any(|call| call.from.name == "outer"));
        let outer = prepare_call_hierarchy(&document, source.find("outer").unwrap()).unwrap();
        let outgoing = outgoing_calls(&document, &outer[0]);
        assert!(outgoing.iter().any(|call| call.to.name == "helper"));
    }
}
