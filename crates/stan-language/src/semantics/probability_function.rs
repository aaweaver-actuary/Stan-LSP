use crate::semantics::symbol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Probability function family exposed through sampling notation.
#[allow(
    missing_docs,
    reason = "variants correspond to Stan density and mass suffixes"
)]
pub enum ProbabilityFunctionKind {
    Density,
    Mass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Semantic link between a sampling name and a user `_lpdf` or `_lpmf` function.
pub struct UserProbabilityFunction {
    /// Sampling-notation name without suffix.
    pub base_name: String,
    /// Underlying user function declaration.
    pub function: symbol::SymbolId,
    /// Continuous-density or discrete-mass family.
    pub kind: ProbabilityFunctionKind,
}
