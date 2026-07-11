//! Conservative per-file symbols, scopes, and references.

use std::collections::BTreeMap;

use crate::{
    Diagnostic, Distribution, Keyword, ProgramBlockKind, StanFunction, StanType, SyntaxKind,
    SyntaxTree, TextRange, Token,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticSymbolKind {
    Data,
    Parameter,
    TransformedData,
    TransformedParameter,
    GeneratedQuantity,
    LocalVariable,
    Function,
    FunctionParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSymbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SemanticSymbolKind,
    pub declaration: TextRange,
    pub name_range: TextRange,
    pub declared_type: Option<StanType>,
    pub scope: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub range: TextRange,
    pub resolved: Option<SymbolId>,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticModel {
    pub symbols: Vec<SemanticSymbol>,
    pub scopes: Vec<Scope>,
    pub references: Vec<Reference>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn analyze_semantics(text: &str, tokens: &[Token], syntax: &SyntaxTree) -> SemanticModel {
    let mut model = SemanticModel {
        scopes: vec![Scope {
            id: ScopeId(0),
            parent: None,
            range: TextRange::new(0, text.len()),
        }],
        ..SemanticModel::default()
    };
    let significant = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            (!matches!(
                token.kind,
                SyntaxKind::Whitespace
                    | SyntaxKind::LineComment
                    | SyntaxKind::BlockComment
                    | SyntaxKind::EndOfFile
            ))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    for node in syntax
        .nodes()
        .iter()
        .filter(|node| node.kind == crate::SyntaxNodeKind::CompoundStatement)
    {
        let parent = model
            .scopes
            .iter()
            .filter(|scope| {
                scope.range.start <= node.range.start
                    && node.range.end <= scope.range.end
                    && scope.range != node.range
            })
            .min_by_key(|scope| scope.range.end - scope.range.start)
            .map(|scope| scope.id)
            .or(Some(ScopeId(0)));
        model.scopes.push(Scope {
            id: ScopeId(model.scopes.len() as u32),
            parent,
            range: node.range,
        });
    }
    let mut declaration_tokens = BTreeMap::<usize, SymbolId>::new();
    for node in syntax
        .nodes()
        .iter()
        .filter(|node| node.kind == crate::SyntaxNodeKind::FunctionDeclaration)
    {
        let function_name = significant
            .iter()
            .copied()
            .filter(|index| node.token_range.contains(index))
            .find(|index| {
                tokens[*index].kind == SyntaxKind::Identifier
                    && significant
                        .iter()
                        .position(|candidate| candidate == index)
                        .and_then(|position| significant.get(position + 1))
                        .is_some_and(|next| {
                            tokens[*next].kind == SyntaxKind::Symbol(crate::Symbol::LeftParen)
                        })
            });
        if let Some(name_index) = function_name {
            let id = SymbolId(model.symbols.len() as u32);
            model.symbols.push(SemanticSymbol {
                id,
                name: token_text(text, tokens[name_index]).to_owned(),
                kind: SemanticSymbolKind::Function,
                declaration: node.range,
                name_range: tokens[name_index].range,
                declared_type: None,
                scope: ScopeId(0),
            });
            declaration_tokens.insert(name_index, id);
        }
    }
    let mut statement_start = 0usize;
    for (position, token_index) in significant.iter().copied().enumerate() {
        if tokens[token_index].kind != SyntaxKind::Symbol(crate::Symbol::Semicolon) {
            continue;
        }
        let statement = &significant[statement_start..position];
        if let Some((name_index, declared_type)) = declaration_in_statement(tokens, statement) {
            let name = token_text(text, tokens[name_index]).to_owned();
            let kind = block_kind_at(syntax, tokens[name_index].range.start)
                .map(symbol_kind_for_block)
                .unwrap_or(SemanticSymbolKind::LocalVariable);
            let id = SymbolId(model.symbols.len() as u32);
            let declaration = TextRange {
                start: statement
                    .first()
                    .map_or(tokens[name_index].range.start, |index| {
                        tokens[*index].range.start
                    }),
                end: tokens[token_index].range.end,
            };
            let scope = if kind == SemanticSymbolKind::LocalVariable {
                scope_at(&model.scopes, tokens[name_index].range.start)
            } else {
                ScopeId(0)
            };
            model.symbols.push(SemanticSymbol {
                id,
                name,
                kind,
                declaration,
                name_range: tokens[name_index].range,
                declared_type: Some(declared_type),
                scope,
            });
            declaration_tokens.insert(name_index, id);
        }
        statement_start = position + 1;
    }

    for (position, token_index) in significant.iter().copied().enumerate() {
        if tokens[token_index].kind != SyntaxKind::Identifier {
            continue;
        }
        if declaration_tokens.contains_key(&token_index) {
            continue;
        }
        let name = token_text(text, tokens[token_index]);
        if StanFunction::from_str(name).is_ok() || Distribution::from_str(name).is_ok() {
            continue;
        }
        let resolved = model
            .symbols
            .iter()
            .filter(|symbol| symbol.name == name)
            .filter(|symbol| {
                symbol.scope == ScopeId(0)
                    || model
                        .scopes
                        .get(symbol.scope.0 as usize)
                        .is_some_and(|scope| {
                            scope.range.start <= tokens[token_index].range.start
                                && tokens[token_index].range.end <= scope.range.end
                        })
            })
            .min_by_key(|symbol| {
                model
                    .scopes
                    .get(symbol.scope.0 as usize)
                    .map_or(u32::MAX, |scope| scope.range.end - scope.range.start)
            })
            .map(|symbol| symbol.id);
        let is_include_path = significant
            .get(position.wrapping_sub(1))
            .is_some_and(|previous| matches!(tokens[*previous].kind, SyntaxKind::Directive(_)));
        if !is_include_path {
            model.references.push(Reference {
                name: name.to_owned(),
                range: tokens[token_index].range,
                resolved,
            });
        }
    }

    let mut by_name = BTreeMap::<(ScopeId, &str), &SemanticSymbol>::new();
    for symbol in &model.symbols {
        if let Some(previous) = by_name.insert((symbol.scope, &symbol.name), symbol) {
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
        }
    }
    model
}

fn scope_at(scopes: &[Scope], offset: u32) -> ScopeId {
    scopes
        .iter()
        .filter(|scope| scope.range.start <= offset && offset <= scope.range.end)
        .min_by_key(|scope| scope.range.end - scope.range.start)
        .map_or(ScopeId(0), |scope| scope.id)
}

fn declaration_in_statement(tokens: &[Token], statement: &[usize]) -> Option<(usize, StanType)> {
    let type_position = statement.iter().position(|index| {
        matches!(
            tokens[*index].kind,
            SyntaxKind::Keyword(
                Keyword::Int
                    | Keyword::Real
                    | Keyword::Complex
                    | Keyword::Vector
                    | Keyword::RowVector
                    | Keyword::Matrix
                    | Keyword::ComplexVector
                    | Keyword::ComplexRowVector
                    | Keyword::ComplexMatrix
                    | Keyword::Array
            )
        )
    })?;
    let declared_type = match tokens[statement[type_position]].kind {
        SyntaxKind::Keyword(Keyword::Int) => StanType::Int,
        SyntaxKind::Keyword(Keyword::Real) => StanType::Real,
        SyntaxKind::Keyword(Keyword::Complex) => StanType::Complex,
        SyntaxKind::Keyword(Keyword::Vector) => StanType::Vector,
        SyntaxKind::Keyword(Keyword::RowVector) => StanType::RowVector,
        SyntaxKind::Keyword(Keyword::Matrix) => StanType::Matrix,
        SyntaxKind::Keyword(Keyword::ComplexVector) => StanType::ComplexVector,
        SyntaxKind::Keyword(Keyword::ComplexRowVector) => StanType::ComplexRowVector,
        SyntaxKind::Keyword(Keyword::ComplexMatrix) => StanType::ComplexMatrix,
        SyntaxKind::Keyword(Keyword::Array) => StanType::TypeVariable("array".to_owned()),
        _ => return None,
    };
    let before_initializer = statement
        .iter()
        .position(|index| tokens[*index].kind == SyntaxKind::Symbol(crate::Symbol::Assign))
        .unwrap_or(statement.len());
    let name = statement[type_position + 1..before_initializer]
        .iter()
        .rev()
        .find(|index| tokens[**index].kind == SyntaxKind::Identifier)
        .copied()?;
    Some((name, declared_type))
}

fn block_kind_at(syntax: &SyntaxTree, offset: u32) -> Option<ProgramBlockKind> {
    syntax.source_file().program_blocks().find_map(|block| {
        let range = block.range();
        (range.start <= offset && offset <= range.end).then_some(block.kind())
    })
}

fn symbol_kind_for_block(kind: ProgramBlockKind) -> SemanticSymbolKind {
    match kind {
        ProgramBlockKind::Data => SemanticSymbolKind::Data,
        ProgramBlockKind::Parameters => SemanticSymbolKind::Parameter,
        ProgramBlockKind::TransformedData => SemanticSymbolKind::TransformedData,
        ProgramBlockKind::TransformedParameters => SemanticSymbolKind::TransformedParameter,
        ProgramBlockKind::GeneratedQuantities => SemanticSymbolKind::GeneratedQuantity,
        _ => SemanticSymbolKind::LocalVariable,
    }
}

fn token_text(text: &str, token: Token) -> &str {
    &text[token.range.start as usize..token.range.end as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn classifies_declarations_and_resolves_references() {
        let analysis =
            analyze("data { real y; } parameters { real theta; } model { y ~ normal(theta, 1); }");
        assert_eq!(analysis.semantics.symbols.len(), 2);
        assert_eq!(analysis.semantics.symbols[0].kind, SemanticSymbolKind::Data);
        assert!(
            analysis
                .semantics
                .references
                .iter()
                .all(|reference| reference.resolved.is_some())
        );
    }
}
