use std::collections::BTreeSet;

use stan_language::{
    CallContext, DataQualifier, Distribution, FunctionCatalog, FunctionCategory, FunctionSignature,
    Lifecycle, STAN_VERSION, StanFunction,
};

#[test]
fn all_function_and_distribution_names_round_trip() {
    let mut names = BTreeSet::new();
    for function in StanFunction::ALL {
        assert!(
            names.insert(function.as_str()),
            "duplicate function {}",
            function.as_str()
        );
        assert_eq!(
            StanFunction::from_str(function.as_str()).unwrap(),
            *function
        );
    }
    for distribution in Distribution::ALL {
        assert_eq!(
            Distribution::from_str(distribution.as_str()).unwrap(),
            *distribution
        );
    }
}

#[test]
fn embedded_catalog_is_complete_and_typed() {
    let catalog = FunctionCatalog::global();
    assert!(catalog.functions().count() > 600);

    for function in StanFunction::ALL {
        let metadata = catalog.metadata(*function);
        assert!(
            !metadata.categories.is_empty(),
            "{} is uncategorized",
            function
        );
        if metadata.lifecycle.is_available_in(STAN_VERSION) {
            assert!(
                !catalog.signatures(*function).is_empty(),
                "{} has no concrete or special signature",
                function,
            );
        }
        for signature in catalog.signatures(*function) {
            match signature {
                FunctionSignature::Concrete(signature) => {
                    assert!(signature.parameters.len() < 64);
                }
                FunctionSignature::Variadic(signature) => {
                    assert!(signature.forwards_arguments);
                    assert!(!signature.display.is_empty());
                    assert!(signature.callback.is_some());
                    if function.as_str().contains("_tol") {
                        assert!(
                            signature.control_parameters.iter().all(|parameter| {
                                parameter.qualifier == DataQualifier::DataOnly
                            })
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn higher_order_special_cases_are_classified() {
    for token in [
        "reduce_sum",
        "ode_rk45",
        "dae",
        "solve_newton",
        "laplace_marginal",
    ] {
        let function = StanFunction::from_str(token).unwrap();
        assert!(
            function
                .metadata()
                .categories
                .contains(&FunctionCategory::HigherOrder)
        );
        assert!(
            function
                .signatures()
                .iter()
                .any(|signature| { matches!(signature, FunctionSignature::Variadic(_)) })
        );
    }
}

#[test]
fn restricted_suffixes_have_restricted_contexts() {
    let catalog = FunctionCatalog::global();
    for function in StanFunction::ALL {
        let contexts = catalog.metadata(*function).call_contexts;
        let name = function.as_str();
        if name.ends_with("_rng") {
            assert!(contexts.contains(CallContext::TransformedData));
            assert!(contexts.contains(CallContext::GeneratedQuantities));
        }
        if name.ends_with("_lupdf") || name.ends_with("_lupmf") {
            assert!(contexts.contains(CallContext::Model));
        }
        if name.ends_with("_jacobian") {
            assert!(contexts.contains(CallContext::TransformedParameters));
        }
    }
}

#[test]
fn removed_functions_are_not_available_in_239() {
    for token in [
        "multiply_log",
        "binomial_coefficient_log",
        "get_lp",
        "fabs",
        "cov_exp_quad",
    ] {
        let function = StanFunction::from_str(token).unwrap();
        assert!(matches!(
            function.metadata().lifecycle,
            Lifecycle::Removed { .. }
        ));
        assert!(!function.metadata().lifecycle.is_available_in(STAN_VERSION));
    }
}

#[test]
fn manual_introduction_versions_are_preserved() {
    assert!(matches!(
        StanFunction::Abs.metadata().lifecycle,
        Lifecycle::Active {
            introduced: Some(version)
        } if version == stan_language::StanVersion::new(2, 0, 0)
    ));
    assert!(matches!(
        StanFunction::LaplaceMarginal.metadata().lifecycle,
        Lifecycle::Active {
            introduced: Some(version)
        } if version == stan_language::StanVersion::new(2, 39, 0)
    ));
}

#[test]
fn catalog_selection_is_explicit_and_versioned() {
    assert!(FunctionCatalog::for_version(STAN_VERSION).is_some());
    assert!(FunctionCatalog::for_version(stan_language::StanVersion::new(2, 38, 0)).is_none());
    assert_eq!(FunctionCatalog::supported_versions(), &[STAN_VERSION]);
}
