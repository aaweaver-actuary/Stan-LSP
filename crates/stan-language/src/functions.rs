use std::{fmt, str::FromStr};

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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseStanVersionError {
    #[error("Stan versions must contain exactly three dot-separated integer components")]
    InvalidFormat,
    #[error("invalid Stan version component {0:?}")]
    InvalidComponent(String),
}

impl FromStr for StanVersion {
    type Err = ParseStanVersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut components = value.split('.');
        let mut parse = || {
            let component = components
                .next()
                .ok_or(ParseStanVersionError::InvalidFormat)?;
            component
                .parse::<u16>()
                .map_err(|_| ParseStanVersionError::InvalidComponent(component.to_owned()))
        };
        let version = Self::new(parse()?, parse()?, parse()?);
        if components.next().is_some() {
            return Err(ParseStanVersionError::InvalidFormat);
        }
        Ok(version)
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
    pub const EMPTY: Self = Self(0);
    pub const TRANSFORMED_DATA: Self = Self(1 << 0);
    pub const TRANSFORMED_PARAMETERS: Self = Self(1 << 1);
    pub const MODEL: Self = Self(1 << 2);
    pub const GENERATED_QUANTITIES: Self = Self(1 << 3);
    pub const RNG_FUNCTION: Self = Self(1 << 4);
    pub const LOG_PROBABILITY_FUNCTION: Self = Self(1 << 5);
    pub const JACOBIAN_FUNCTION: Self = Self(1 << 6);
    pub const ANY: Self = Self(
        Self::TRANSFORMED_DATA.0
            | Self::TRANSFORMED_PARAMETERS.0
            | Self::MODEL.0
            | Self::GENERATED_QUANTITIES.0
            | Self::RNG_FUNCTION.0
            | Self::LOG_PROBABILITY_FUNCTION.0
            | Self::JACOBIAN_FUNCTION.0,
    );

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, context: CallContext) -> bool {
        let mask = match context {
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

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXTS: [CallContext; 7] = [
        CallContext::TransformedData,
        CallContext::TransformedParameters,
        CallContext::Model,
        CallContext::GeneratedQuantities,
        CallContext::RngFunction,
        CallContext::LogProbabilityFunction,
        CallContext::JacobianFunction,
    ];

    #[test]
    fn any_contains_every_concrete_context() {
        assert!(
            CONTEXTS
                .into_iter()
                .all(|context| CallContextSet::ANY.contains(context))
        );
        assert!(
            CONTEXTS
                .into_iter()
                .all(|context| !CallContextSet::EMPTY.contains(context))
        );
    }

    #[test]
    fn restricted_context_sets_remain_restricted() {
        assert!(CallContextSet::MODEL.contains(CallContext::Model));
        assert!(!CallContextSet::MODEL.contains(CallContext::GeneratedQuantities));
        assert_eq!(
            CallContextSet::MODEL.union(CallContextSet::MODEL),
            CallContextSet::MODEL
        );
    }

    #[test]
    fn versions_parse_order_and_round_trip() {
        let version: StanVersion = "2.39.0".parse().unwrap();
        assert_eq!(version, StanVersion::new(2, 39, 0));
        assert_eq!(version.to_string(), "2.39.0");
        assert!(StanVersion::new(2, 40, 0) > version);
        assert!("2.39".parse::<StanVersion>().is_err());
        assert!("2.39.0.1".parse::<StanVersion>().is_err());
    }
}
