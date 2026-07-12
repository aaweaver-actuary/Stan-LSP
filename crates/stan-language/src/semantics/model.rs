use std::collections::{BTreeMap, BTreeSet};

use crate::{Diagnostic, StanType, TextRange, TextSize};

use super::{
    SemanticInvariantError,
    probability_function::UserProbabilityFunction,
    reference::{Reference, ReferenceId},
    resolution::resolve_name,
    scope::{Scope, ScopeId, ScopeKind},
    symbol::{SemanticSymbol, SemanticSymbolKind, SymbolId},
};

#[derive(Debug, Clone, Default)]
/// Conservative symbols, scopes, references, user distributions, and known types.
pub struct SemanticModel {
    /// Declarations in stable source-discovery order.
    pub symbols: Vec<SemanticSymbol>,
    /// Lexical scopes with root at index zero.
    pub scopes: Vec<Scope>,
    /// Identifier uses in source order.
    pub references: Vec<Reference>,
    /// User-defined probability functions indexed once per snapshot.
    pub user_probability_functions: Vec<UserProbabilityFunction>,
    /// Verified semantic errors such as same-scope duplicates.
    pub diagnostics: Vec<Diagnostic>,
    /// Locally known types keyed by exact expression/name range.
    pub inferred_types: BTreeMap<TextRange, StanType>,
}

impl SemanticModel {
    /// Returns the declaration selected for `reference`, if any.
    ///
    /// Gets the first declaration in source order that is visible at the reference's offset.
    ///
    /// # Arguments
    ///
    /// * `reference` - The identifier use to resolve.
    ///
    /// # Returns
    ///
    /// * `Some(SymbolId)` - The declaration that is visible at the reference's offset.
    /// * `None` - If no declaration is visible at the reference's offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use stan_language::{analyze_revision, SemanticModel, ReferenceId, Revision};
    /// let source = "data { real y; } model { y ~ normal(0, 1); }";
    /// let analysis = stan_language::analyze_revision(source, Revision::default());
    /// let reference_id = ReferenceId(0); // Assuming the first reference is the one we want to resolve
    ///
    /// let actual_resolved_symbol = analysis.semantics.resolve_reference(reference_id);
    /// let expected_symbol_id = analysis.semantics.references[0].resolved;
    ///
    /// assert_eq!(actual_resolved_symbol, expected_symbol_id);
    ///
    /// ```
    ///
    pub fn resolve_reference(&self, reference: ReferenceId) -> Option<SymbolId> {
        self.references
            .get(reference.0 as usize)
            .filter(|candidate| candidate.id == reference)
            .and_then(|candidate| candidate.resolved)
    }

    /// Finds the declaration or resolved reference at a UTF-8 byte offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - The byte offset in the source text to query.
    ///
    /// # Returns
    ///
    /// * `Some(&SemanticSymbol)` - The declaration or resolved reference at the given offset.
    /// * `None` - If no declaration or resolved reference is found at the given offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use stan_language::{analyze_revision, SemanticModel, SemanticSymbolKind, Revision};
    ///
    /// let source = "data { real y; } parameters { real theta; } model { y ~ normal(theta, 1); }";
    /// let analysis = analyze_revision(source, Revision::default());
    /// let byte_offset = source.find("theta").unwrap() as u32; // Find the offset of "theta"
    ///
    /// let symbol_at_offset = analysis.semantics.symbol_at(byte_offset);
    /// assert!(symbol_at_offset.is_some());
    ///
    /// let symbol = symbol_at_offset.unwrap();
    /// assert_eq!(symbol.name, "theta");
    /// assert_eq!(symbol.kind, SemanticSymbolKind::Parameter);
    /// ```
    pub fn symbol_at(&self, offset: TextSize) -> Option<&SemanticSymbol> {
        if let Some(symbol) = self
            .symbols
            .iter()
            .find(|symbol| symbol.name_range.contains(offset))
        {
            return Some(symbol);
        }
        self.references
            .iter()
            .find(|reference| reference.range.contains(offset))
            .and_then(|reference| reference.resolved)
            .and_then(|id| self.symbols.get(id.0 as usize))
    }

    /// Enumerates references resolved to `symbol` in source order.
    pub fn references_to(&self, symbol: SymbolId) -> impl Iterator<Item = &Reference> {
        self.references
            .iter()
            .filter(move |reference| reference.resolved == Some(symbol))
    }

    /// Enumerates user probability functions exposed as `base_name`.
    pub fn probability_functions(
        &self,
        base_name: &str,
    ) -> impl Iterator<Item = &UserProbabilityFunction> {
        self.user_probability_functions
            .iter()
            .filter(move |function| function.base_name == base_name)
    }

    /// Returns declarations conservatively visible at `offset`.
    pub fn visible_symbols_at(&self, offset: TextSize) -> Vec<&SemanticSymbol> {
        let scope = self
            .scopes
            .iter()
            .filter(|scope| scope.range.contains(offset))
            .min_by_key(|scope| scope.range.end - scope.range.start)
            .map_or(ScopeId(0), |scope| scope.id);
        let mut by_name = BTreeMap::new();
        for symbol in &self.symbols {
            if let Some(resolved) = resolve_name(self, &symbol.name, scope, offset) {
                by_name.entry(symbol.name.as_str()).or_insert(resolved);
            }
        }
        by_name
            .into_values()
            .filter_map(|id| self.symbols.get(id.0 as usize))
            .collect()
    }

    /// Checks IDs, parentage, containment, binder scopes, and resolutions.
    ///
    /// This debug/test API reports implementation defects, not source errors.
    pub fn validate(&self) -> Result<(), SemanticInvariantError> {
        let contains_range = |outer: TextRange, inner: TextRange| {
            outer.start <= inner.start && inner.end <= outer.end
        };
        if self.scopes.first().is_none_or(|scope| {
            scope.id != ScopeId(0) || scope.parent.is_some() || scope.kind != ScopeKind::Root
        }) {
            return Err(SemanticInvariantError::MissingRootScope);
        }
        let mut scope_ids = BTreeSet::new();
        for scope in &self.scopes {
            if scope.id.0 as usize >= self.scopes.len() {
                return Err(SemanticInvariantError::InvalidScopeId(scope.id));
            }
            if !scope_ids.insert(scope.id) {
                return Err(SemanticInvariantError::DuplicateScopeId(scope.id));
            }
            if let Some(parent) = scope.parent {
                let Some(parent_scope) = self.scopes.get(parent.0 as usize) else {
                    return Err(SemanticInvariantError::InvalidScopeParent(scope.id));
                };
                if !contains_range(parent_scope.range, scope.range) {
                    return Err(SemanticInvariantError::ScopeOutsideParent(scope.id));
                }
            }
        }
        let mut symbol_ids = BTreeSet::new();
        for symbol in &self.symbols {
            if symbol.id.0 as usize >= self.symbols.len() {
                return Err(SemanticInvariantError::InvalidSymbolId(symbol.id));
            }
            if !symbol_ids.insert(symbol.id) {
                return Err(SemanticInvariantError::DuplicateSymbolId(symbol.id));
            }
            let Some(scope) = self.scopes.get(symbol.scope.0 as usize) else {
                return Err(SemanticInvariantError::MissingSymbolScope(symbol.id));
            };
            if symbol.scope != ScopeId(0) && !contains_range(scope.range, symbol.name_range) {
                return Err(SemanticInvariantError::SymbolOutsideScope(symbol.id));
            }
            if matches!(
                symbol.kind,
                SemanticSymbolKind::FunctionParameter | SemanticSymbolKind::LoopVariable
            ) && !matches!(scope.kind, ScopeKind::Function | ScopeKind::Loop)
            {
                return Err(SemanticInvariantError::InvalidBinderScope(symbol.id));
            }
        }
        let mut reference_ids = BTreeSet::new();
        for reference in &self.references {
            if reference.id.0 as usize >= self.references.len() {
                return Err(SemanticInvariantError::InvalidReferenceId(reference.id));
            }
            if !reference_ids.insert(reference.id) {
                return Err(SemanticInvariantError::DuplicateReferenceId(reference.id));
            }
            if self.scopes.get(reference.scope.0 as usize).is_none() {
                return Err(SemanticInvariantError::MissingReferenceScope(reference.id));
            }
            if let Some(symbol) = reference.resolved {
                if self.symbols.get(symbol.0 as usize).is_none() {
                    return Err(SemanticInvariantError::MissingResolvedSymbol(
                        reference.id,
                        symbol,
                    ));
                }
            }
        }
        Ok(())
    }
}
