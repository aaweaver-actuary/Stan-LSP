use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, Distribution, ProgramBlockKind, Scope, SemanticSymbolKind, StanFunction, StanType,
    SyntaxKind, SyntaxTree, TextRange, TextSize, Token, TypeSyntax,
    semantics::{model, probability_function, reference, resolution, scope, symbol},
};
use probability_function::ProbabilityFunctionKind;

/// Builds conservative per-file semantics from structured syntax.
///
/// Unsupported or incomplete constructs remain unresolved rather than producing
/// speculative bindings.
pub fn analyze_semantics(
    text: &str,
    tokens: &[Token],
    syntax: &SyntaxTree,
) -> model::SemanticModel {
    let mut model = model::SemanticModel {
        scopes: build_scopes(text, syntax),
        ..model::SemanticModel::default()
    };
    let mut declaration_ranges = BTreeSet::<TextRange>::new();

    for function in syntax.source_file().function_declarations() {
        // Skip functions without a name, which are invalid and will be
        // reported by validation.
        let Some(name) = function.name() else {
            continue;
        };

        // Add the function symbol to the model
        // ID representing the function symbol is returned and stored in `id`
        let id = add_symbol(
            &mut model,
            name.text(),
            SemanticSymbolKind::Function,
            function.range(),
            name.range(),
            function.return_type().and_then(parse_type_syntax),
            scope::ScopeId(0),
            0,
        );

        declaration_ranges.insert(name.range());
        if let Some(body) = function.body_range() {
            if let Some(scope) = scope_with_range(&model.scopes, function.range()) {
                for parameter in function.parameters() {
                    let name = parameter.name();
                    add_symbol(
                        &mut model,
                        name.text(),
                        SemanticSymbolKind::FunctionParameter,
                        parameter.range(),
                        name.range(),
                        parse_type_syntax(parameter.type_syntax()),
                        scope,
                        body.start,
                    );
                    declaration_ranges.insert(name.range());
                }
            }
        }
        if let Some((base_name, kind)) = probability_name(name.text()) {
            model
                .user_probability_functions
                .push(probability_function::UserProbabilityFunction {
                    base_name: base_name.to_owned(),
                    function: id,
                    kind,
                });
        }
    }

    for declaration in syntax.source_file().variable_declarations() {
        let declared_type = declaration.type_syntax().and_then(parse_type_syntax);
        for declarator in declaration.declarators() {
            let name = declarator.name();
            let containing_scope = scope_at(&model.scopes, name.range().start);
            let block_kind = block_kind_at(syntax, name.range().start);
            let top_level_block = matches!(
                model.scopes[containing_scope.0 as usize].kind,
                scope::ScopeKind::ProgramBlock(_)
            );
            let kind = if top_level_block {
                block_kind
                    .map(symbol_kind_for_block)
                    .unwrap_or(SemanticSymbolKind::LocalVariable)
            } else {
                SemanticSymbolKind::LocalVariable
            };
            let scope = if top_level_block && is_program_symbol(kind) {
                scope::ScopeId(0)
            } else {
                containing_scope
            };
            add_symbol(
                &mut model,
                name.text(),
                kind,
                declaration.range(),
                name.range(),
                declared_type.clone(),
                scope,
                declaration.range().end,
            );
            declaration_ranges.insert(name.range());
        }
    }

    for for_statement in syntax.source_file().for_statements() {
        let (Some(binder), Some(body)) = (for_statement.binder(), for_statement.body_range())
        else {
            continue;
        };
        let Some(scope) = scope_with_range(&model.scopes, for_statement.range()) else {
            continue;
        };
        add_symbol(
            &mut model,
            binder.text(),
            SemanticSymbolKind::LoopVariable,
            for_statement.range(),
            binder.range(),
            Some(StanType::Int),
            scope,
            body.start,
        );
        declaration_ranges.insert(binder.range());
    }

    emit_duplicate_declarations(&mut model);
    resolve_references(text, tokens, &declaration_ranges, &mut model);
    infer_literal_types(tokens, &mut model);

    #[cfg(debug_assertions)]
    debug_assert!(
        model.validate().is_ok(),
        "invalid semantic model: {:?}",
        model.validate()
    );
    model
}

fn build_scopes(text: &str, syntax: &SyntaxTree) -> Vec<scope::Scope> {
    let mut scoped_ranges = syntax
        .source_file()
        .compound_statements()
        .map(|statement| (statement.range(), scope::ScopeKind::Block))
        .collect::<Vec<_>>();
    scoped_ranges.extend(
        syntax
            .source_file()
            .function_declarations()
            .map(|function| (function.range(), scope::ScopeKind::Function)),
    );
    scoped_ranges.extend(
        syntax
            .source_file()
            .for_statements()
            .map(|statement| (statement.range(), scope::ScopeKind::Loop)),
    );
    scoped_ranges
        .sort_by_key(|(range, _)| (range.start, std::cmp::Reverse(range.end - range.start)));
    scoped_ranges.dedup_by_key(|(range, _)| *range);
    let blocks = syntax
        .source_file()
        .program_blocks()
        .map(|block| (block.kind(), block.range()))
        .collect::<Vec<_>>();
    let mut scopes = vec![scope::Scope {
        id: scope::ScopeId(0),
        parent: None,
        range: TextRange::new(0, text.len()),
        kind: scope::ScopeKind::Root,
    }];
    for (range, requested_kind) in scoped_ranges {
        let parent = scopes
            .iter()
            .filter(|scope| scope.range != range && contains_range(scope.range, range))
            .min_by_key(|scope| scope.range.end - scope.range.start)
            .map_or(scope::ScopeId(0), |scope| scope.id);
        let kind = if requested_kind != scope::ScopeKind::Block {
            requested_kind
        } else if parent == scope::ScopeId(0) {
            blocks
                .iter()
                .find_map(|(kind, block_range)| {
                    contains_range(*block_range, range)
                        .then_some(scope::ScopeKind::ProgramBlock(*kind))
                })
                .unwrap_or(scope::ScopeKind::Block)
        } else {
            scope::ScopeKind::Block
        };
        scopes.push(scope::Scope {
            id: scope::ScopeId(scopes.len() as u32),
            parent: Some(parent),
            range,
            kind,
        });
    }
    scopes
}

fn emit_duplicate_declarations(model: &mut model::SemanticModel) {
    let mut first_by_name = BTreeMap::<(scope::ScopeId, String), usize>::new();
    for index in 0..model.symbols.len() {
        let symbol = &model.symbols[index];
        let key = (symbol.scope, symbol.name.clone());
        if let Some(previous_index) = first_by_name.get(&key).copied() {
            let previous = &model.symbols[previous_index];
            let mut diagnostic = Diagnostic::error(
                "semantic.duplicate-declaration",
                format!("duplicate declaration of `{}`", symbol.name),
                symbol.name_range,
            );
            diagnostic.related.push(crate::RelatedDiagnostic {
                range: previous.name_range,
                message: "first declaration is here".to_owned(),
            });
            model.diagnostics.push(diagnostic);
        } else {
            first_by_name.insert(key, index);
        }
    }
}

fn infer_literal_types(tokens: &[Token], model: &mut model::SemanticModel) {
    for token in tokens {
        let inferred = match token.kind {
            SyntaxKind::IntegerLiteral => Some(StanType::Int),
            SyntaxKind::RealLiteral => Some(StanType::Real),
            SyntaxKind::ImaginaryLiteral => Some(StanType::Complex),
            _ => None,
        };
        if let Some(inferred) = inferred {
            model.inferred_types.insert(token.range, inferred);
        }
    }
}

fn contains_range(outer: TextRange, inner: TextRange) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

#[allow(clippy::too_many_arguments)]
fn add_symbol(
    model: &mut model::SemanticModel,
    name: &str,
    kind: SemanticSymbolKind,
    declaration: TextRange,
    name_range: TextRange,
    declared_type: Option<StanType>,
    scope: scope::ScopeId,
    visible_from: TextSize,
) -> symbol::SymbolId {
    let id = symbol::SymbolId(model.symbols.len() as u32);
    model.symbols.push(symbol::SemanticSymbol {
        id,
        name: name.to_owned(),
        kind,
        declaration,
        name_range,
        declared_type,
        scope,
        visible_from,
    });
    id
}

fn resolve_references(
    text: &str,
    tokens: &[Token],
    declaration_ranges: &BTreeSet<TextRange>,
    model: &mut model::SemanticModel,
) {
    for (position, token) in tokens.iter().copied().enumerate() {
        if token.kind != SyntaxKind::Identifier || declaration_ranges.contains(&token.range) {
            continue;
        }
        if position
            .checked_sub(1)
            .is_some_and(|previous| matches!(tokens[previous].kind, SyntaxKind::Directive(_)))
        {
            continue;
        }
        let name = token_text(text, token);
        let scope = scope_at(&model.scopes, token.range.start);
        let mut resolved = resolution::resolve_name(model, name, scope, token.range.start);
        if resolved.is_none() {
            let probability_matches = model
                .probability_functions(name)
                .map(|function| function.function)
                .collect::<Vec<_>>();
            if probability_matches.len() == 1 {
                resolved = probability_matches.first().copied();
            }
        }
        if resolved.is_none()
            && (StanFunction::from_str(name).is_ok() || Distribution::from_str(name).is_ok())
        {
            continue;
        }
        let id = reference::ReferenceId(model.references.len() as u32);
        model.references.push(reference::Reference {
            id,
            name: name.to_owned(),
            range: token.range,
            scope,
            resolved,
        });
        if let Some(r#type) = resolved
            .and_then(|id| model.symbols.get(id.0 as usize))
            .and_then(|symbol| symbol.declared_type.clone())
        {
            model.inferred_types.insert(token.range, r#type);
        }
    }
}

fn parse_type_syntax(syntax: TypeSyntax<'_>) -> Option<StanType> {
    let text = syntax.text().trim();
    let dimensions = text
        .strip_prefix("array")
        .and_then(|rest| rest.trim_start().strip_prefix('['))
        .and_then(|rest| rest.split_once(']'))
        .map(|(dimensions, rest)| {
            let count = dimensions.bytes().filter(|byte| *byte == b',').count() + 1;
            (count, rest.trim())
        });
    if let Some((dimensions, element)) = dimensions {
        let element = primitive_type(element)?;
        return Some(StanType::Array {
            dimensions,
            element: Box::new(element),
        });
    }
    primitive_type(text)
}

fn primitive_type(text: &str) -> Option<StanType> {
    let word = text
        .split(|character: char| character == '<' || character == '[' || character.is_whitespace())
        .find(|part| !part.is_empty())?;
    match word {
        "int" => Some(StanType::Int),
        "real" => Some(StanType::Real),
        "complex" => Some(StanType::Complex),
        "vector" => Some(StanType::Vector),
        "row_vector" => Some(StanType::RowVector),
        "matrix" => Some(StanType::Matrix),
        "complex_vector" => Some(StanType::ComplexVector),
        "complex_row_vector" => Some(StanType::ComplexRowVector),
        "complex_matrix" => Some(StanType::ComplexMatrix),
        "tuple" => Some(StanType::TypeVariable(text.to_owned())),
        "void" => None,
        _ => None,
    }
}

fn probability_name(name: &str) -> Option<(&str, ProbabilityFunctionKind)> {
    name.strip_suffix("_lpdf")
        .map(|base| (base, ProbabilityFunctionKind::Density))
        .or_else(|| {
            name.strip_suffix("_lpmf")
                .map(|base| (base, ProbabilityFunctionKind::Mass))
        })
}

fn scope_at(scopes: &[Scope], offset: TextSize) -> scope::ScopeId {
    scopes
        .iter()
        .filter(|scope| scope.range.contains(offset))
        .min_by_key(|scope| scope.range.end - scope.range.start)
        .map_or(scope::ScopeId(0), |scope| scope.id)
}

fn scope_with_range(scopes: &[Scope], range: TextRange) -> Option<scope::ScopeId> {
    scopes
        .iter()
        .find(|scope| scope.range == range)
        .map(|scope| scope.id)
}

fn block_kind_at(syntax: &SyntaxTree, offset: TextSize) -> Option<ProgramBlockKind> {
    syntax
        .source_file()
        .program_blocks()
        .find_map(|block| block.range().contains(offset).then_some(block.kind()))
}

fn symbol_kind_for_block(kind: ProgramBlockKind) -> SemanticSymbolKind {
    match kind {
        ProgramBlockKind::Data => SemanticSymbolKind::Data,
        ProgramBlockKind::Parameters => SemanticSymbolKind::Parameter,
        ProgramBlockKind::TransformedData => SemanticSymbolKind::TransformedData,
        ProgramBlockKind::TransformedParameters => SemanticSymbolKind::TransformedParameter,
        ProgramBlockKind::GeneratedQuantities => SemanticSymbolKind::GeneratedQuantity,
        ProgramBlockKind::Functions | ProgramBlockKind::Model => SemanticSymbolKind::LocalVariable,
    }
}

/// Returns true if the symbol kind is one of the program-level symbols (data, parameter, transformed data, transformed parameter, generated quantity).
fn is_program_symbol(kind: SemanticSymbolKind) -> bool {
    matches!(
        kind,
        SemanticSymbolKind::Data
            | SemanticSymbolKind::Parameter
            | SemanticSymbolKind::TransformedData
            | SemanticSymbolKind::TransformedParameter
            | SemanticSymbolKind::GeneratedQuantity
    )
}

fn token_text(text: &str, token: Token) -> &str {
    &text[token.range.start as usize..token.range.end as usize]
}
