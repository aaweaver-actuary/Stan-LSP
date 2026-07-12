//! Declaration-order and lexical-ancestry name lookup.

use crate::{ProgramBlockKind, TextSize};

use super::{
    model::SemanticModel,
    scope::{Scope, ScopeId, ScopeKind},
    symbol::{SemanticSymbolKind, SymbolId},
};

pub(super) fn resolve_name(
    model: &SemanticModel,
    name: &str,
    reference_scope: ScopeId,
    offset: TextSize,
) -> Option<SymbolId> {
    let inside_function =
        scope_chain(model, reference_scope).any(|scope| scope.kind == ScopeKind::Function);
    let reference_block = scope_chain(model, reference_scope).find_map(|scope| match scope.kind {
        ScopeKind::ProgramBlock(kind) => Some(kind),
        _ => None,
    });
    for scope in scope_chain(model, reference_scope) {
        let candidate = model
            .symbols
            .iter()
            .filter(|symbol| symbol.scope == scope.id && symbol.name == name)
            .filter(|symbol| symbol.visible_from <= offset)
            .filter(|symbol| {
                if symbol.kind == SemanticSymbolKind::Function {
                    return true;
                }
                if symbol.scope != ScopeId(0) {
                    return true;
                }
                !inside_function
                    && reference_block
                        .is_some_and(|block| program_symbol_visible(symbol.kind, block))
            })
            .max_by_key(|symbol| symbol.visible_from);
        if let Some(candidate) = candidate {
            return Some(candidate.id);
        }
    }
    None
}

fn scope_chain(model: &SemanticModel, start: ScopeId) -> impl Iterator<Item = &Scope> {
    std::iter::successors(model.scopes.get(start.0 as usize), move |scope| {
        scope
            .parent
            .and_then(|parent| model.scopes.get(parent.0 as usize))
    })
}

fn program_symbol_visible(kind: SemanticSymbolKind, block: ProgramBlockKind) -> bool {
    match kind {
        SemanticSymbolKind::Data => !matches!(block, ProgramBlockKind::Functions),
        SemanticSymbolKind::TransformedData => matches!(
            block,
            ProgramBlockKind::TransformedData
                | ProgramBlockKind::Parameters
                | ProgramBlockKind::TransformedParameters
                | ProgramBlockKind::Model
                | ProgramBlockKind::GeneratedQuantities
        ),
        SemanticSymbolKind::Parameter => matches!(
            block,
            ProgramBlockKind::Parameters
                | ProgramBlockKind::TransformedParameters
                | ProgramBlockKind::Model
                | ProgramBlockKind::GeneratedQuantities
        ),
        SemanticSymbolKind::TransformedParameter => matches!(
            block,
            ProgramBlockKind::TransformedParameters
                | ProgramBlockKind::Model
                | ProgramBlockKind::GeneratedQuantities
        ),
        SemanticSymbolKind::GeneratedQuantity => block == ProgramBlockKind::GeneratedQuantities,
        _ => false,
    }
}
