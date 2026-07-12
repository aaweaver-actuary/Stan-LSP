//! Conservative per-file symbols, lexical scopes, and references.

pub mod analyze_semantics;
pub mod model;
pub mod probability_function;
pub mod reference;
pub mod resolution;
pub mod scope;
pub mod symbol;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Internal consistency failure returned by [`model::SemanticModel::validate`].
#[allow(
    missing_docs,
    reason = "variants precisely name each validated invariant"
)]
pub enum SemanticInvariantError {
    MissingRootScope,
    InvalidScopeId(scope::ScopeId),
    InvalidScopeParent(scope::ScopeId),
    ScopeOutsideParent(scope::ScopeId),
    DuplicateScopeId(scope::ScopeId),
    InvalidSymbolId(symbol::SymbolId),
    DuplicateSymbolId(symbol::SymbolId),
    MissingSymbolScope(symbol::SymbolId),
    SymbolOutsideScope(symbol::SymbolId),
    InvalidBinderScope(symbol::SymbolId),
    InvalidReferenceId(reference::ReferenceId),
    DuplicateReferenceId(reference::ReferenceId),
    MissingReferenceScope(reference::ReferenceId),
    MissingResolvedSymbol(reference::ReferenceId, symbol::SymbolId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Revision, SemanticSymbolKind, analyze_revision};

    fn symbol<'a>(
        model: &'a model::SemanticModel,
        name: &str,
        kind: SemanticSymbolKind,
    ) -> &'a symbol::SemanticSymbol {
        model
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.kind == kind)
            .unwrap()
    }

    #[test]
    fn classifies_declarations_and_resolves_references() {
        let analysis = analyze_revision(
            "data { real y; } parameters { real theta; } model { y ~ normal(theta, 1); }",
            Revision::default(),
        );
        assert_eq!(analysis.semantics.symbols.len(), 2);
        assert_eq!(analysis.semantics.symbols[0].kind, SemanticSymbolKind::Data);
        assert!(
            analysis
                .semantics
                .references
                .iter()
                .all(|reference| reference.resolved.is_some())
        );
        assert_eq!(analysis.semantics.validate(), Ok(()));
    }

    #[test]
    fn function_parameters_are_scoped_and_resolved() {
        let source = "functions { real square(real x) { return x * x; } } model { square(2); }";
        let analysis = analyze_revision(source, Revision::default());
        let parameter = symbol(
            &analysis.semantics,
            "x",
            SemanticSymbolKind::FunctionParameter,
        );
        let references = analysis
            .semantics
            .references_to(parameter.id)
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 2);
        assert!(references.iter().all(|reference| source
            [reference.range.start as usize..reference.range.end as usize]
            == *"x"));
        assert_eq!(analysis.semantics.validate(), Ok(()));
    }

    #[test]
    fn loop_binders_are_scoped_and_do_not_escape() {
        let source = "data { int N; array[N] real y; } model { for (n in 1:N) { y[n] ~ normal(n, 1); } n = 2; }";
        let analysis = analyze_revision(source, Revision::default());
        let binder = symbol(&analysis.semantics, "n", SemanticSymbolKind::LoopVariable);
        assert_eq!(analysis.semantics.references_to(binder.id).count(), 2);
        let trailing = analysis
            .semantics
            .references
            .iter()
            .filter(|reference| reference.name == "n")
            .max_by_key(|reference| reference.range.start)
            .unwrap();
        assert_eq!(trailing.resolved, None);
        assert_eq!(analysis.semantics.validate(), Ok(()));
    }

    #[test]
    fn declaration_order_and_shadowing_are_respected() {
        let source = "model { x = 1; real x; { real x; x = 2; } x = 3; }";
        let analysis = analyze_revision(source, Revision::default());
        let x_references = analysis
            .semantics
            .references
            .iter()
            .filter(|reference| reference.name == "x")
            .collect::<Vec<_>>();
        assert_eq!(x_references.len(), 3);
        assert_eq!(x_references[0].resolved, None);
        assert_ne!(x_references[1].resolved, x_references[2].resolved);
        assert_eq!(model::SemanticModel::validate(&analysis.semantics), Ok(()));
    }

    #[test]
    fn user_probability_functions_resolve_sampling_notation() {
        let source = "functions { real custom_lpdf(real y, real theta) { return normal_lpdf(y | theta, 1); } } data { real y; } parameters { real theta; } model { y ~ custom(theta); }";
        let analysis = analyze_revision(source, Revision::default());
        let custom = symbol(
            &analysis.semantics,
            "custom_lpdf",
            SemanticSymbolKind::Function,
        );
        assert_eq!(
            analysis.semantics.probability_functions("custom").count(),
            1
        );
        assert!(
            analysis
                .semantics
                .references_to(custom.id)
                .any(|reference| reference.name == "custom")
        );
        assert_eq!(model::SemanticModel::validate(&analysis.semantics), Ok(()));
    }
}
