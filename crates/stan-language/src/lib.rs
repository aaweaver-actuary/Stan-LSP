//! Versioned vocabulary and built-in function metadata for the Stan language.

mod catalog;
mod functions;
mod syntax;
mod types;

pub use catalog::{FunctionCatalog, FunctionMetadata};
pub use functions::{
    CallContext, CallContextSet, Distribution, DistributionKind, FunctionCategory, Lifecycle,
    StanFunction, StanVersion,
};
pub use syntax::{
    Associativity, Directive, Fixity, Keyword, KeywordRole, LegacyLanguageElement, LexemeKind,
    OperatorForm, Symbol, SymbolRole,
};
pub use types::{
    CallbackSignature, ConcreteSignature, DataQualifier, FunctionSignature, Parameter, ReturnType,
    StanType, VariadicSignature,
};

/// The Stan language version represented by this crate's embedded catalog.
pub const STAN_VERSION: StanVersion = StanVersion::new(2, 39, 0);
