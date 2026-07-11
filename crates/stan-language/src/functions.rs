use std::fmt;

use serde::{Deserialize, Serialize};
use stan_token_derive::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StanVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl StanVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for StanVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunctionCategory {
    ScalarMath,
    ComplexMath,
    Array,
    Matrix,
    ComplexMatrix,
    SparseMatrix,
    Mixed,
    HigherOrder,
    Transform,
    Probability,
    HiddenMarkov,
    EmbeddedLaplace,
    Utility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallContext {
    Any,
    TransformedData,
    TransformedParameters,
    Model,
    GeneratedQuantities,
    RngFunction,
    LogProbabilityFunction,
    JacobianFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallContextSet(u16);

impl CallContextSet {
    pub const ANY: Self = Self(1 << 0);
    pub const TRANSFORMED_DATA: Self = Self(1 << 1);
    pub const TRANSFORMED_PARAMETERS: Self = Self(1 << 2);
    pub const MODEL: Self = Self(1 << 3);
    pub const GENERATED_QUANTITIES: Self = Self(1 << 4);
    pub const RNG_FUNCTION: Self = Self(1 << 5);
    pub const LOG_PROBABILITY_FUNCTION: Self = Self(1 << 6);
    pub const JACOBIAN_FUNCTION: Self = Self(1 << 7);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, context: CallContext) -> bool {
        let mask = match context {
            CallContext::Any => Self::ANY.0,
            CallContext::TransformedData => Self::TRANSFORMED_DATA.0,
            CallContext::TransformedParameters => Self::TRANSFORMED_PARAMETERS.0,
            CallContext::Model => Self::MODEL.0,
            CallContext::GeneratedQuantities => Self::GENERATED_QUANTITIES.0,
            CallContext::RngFunction => Self::RNG_FUNCTION.0,
            CallContext::LogProbabilityFunction => Self::LOG_PROBABILITY_FUNCTION.0,
            CallContext::JacobianFunction => Self::JACOBIAN_FUNCTION.0,
        };
        self.0 & mask != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    Active {
        introduced: Option<StanVersion>,
    },
    Deprecated {
        introduced: Option<StanVersion>,
        deprecated: StanVersion,
        replacement: Option<&'static str>,
    },
    Removed {
        removed: StanVersion,
        replacement: Option<&'static str>,
    },
}

impl Lifecycle {
    pub fn is_available_in(&self, version: StanVersion) -> bool {
        match self {
            Self::Active { introduced } | Self::Deprecated { introduced, .. } => {
                introduced.is_none_or(|introduced| introduced <= version)
            }
            Self::Removed { removed, .. } => {
                (version.major, version.minor, version.patch)
                    < (removed.major, removed.minor, removed.patch)
            }
        }
    }
}

include!("generated_functions.rs");
include!("generated_distributions.rs");
