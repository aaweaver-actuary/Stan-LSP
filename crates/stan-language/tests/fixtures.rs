use serde_json::Value;
use stan_language::{
    FormatterConfig, LintConfig, Revision, SemanticSymbolKind, analyze_revision, format, lint,
};

#[test]
fn semantic_sidecar_matches_exact_symbols_and_references() {
    let source = include_str!("fixtures/semantics/binders.stan");
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/semantics/binders.expected.json")).unwrap();
    let analysis = analyze_revision(source, Revision::default());
    assert_eq!(analysis.semantics.validate(), Ok(()));

    let actual_symbols = analysis
        .semantics
        .symbols
        .iter()
        .map(|symbol| {
            serde_json::json!({
                "name": symbol.name,
                "kind": format!("{:?}", symbol.kind),
                "start": symbol.name_range.start,
                "end": symbol.name_range.end,
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(Value::Array(actual_symbols), expected["symbols"]);

    let actual_references = analysis
        .semantics
        .references
        .iter()
        .map(|reference| {
            let target = reference
                .resolved
                .and_then(|id| analysis.semantics.symbols.get(id.0 as usize))
                .map(|symbol| symbol.name.as_str());
            serde_json::json!({
                "name": reference.name,
                "start": reference.range.start,
                "end": reference.range.end,
                "target": target,
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(Value::Array(actual_references), expected["references"]);
}

#[test]
fn navigation_sidecar_proves_shadowing_bindings() {
    let source = include_str!("fixtures/navigation/shadowing.stan");
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/navigation/shadowing.expected.json")).unwrap();
    let analysis = analyze_revision(source, Revision::default());
    let declarations = analysis
        .semantics
        .symbols
        .iter()
        .filter(|symbol| symbol.name == "x" && symbol.kind == SemanticSymbolKind::LocalVariable)
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 2);
    for (label, symbol) in [("outer", declarations[0]), ("inner", declarations[1])] {
        assert_eq!(
            serde_json::json!([symbol.name_range.start, symbol.name_range.end]),
            expected[label]["declaration"]
        );
        let references = analysis
            .semantics
            .references_to(symbol.id)
            .map(|reference| serde_json::json!([reference.range.start, reference.range.end]))
            .collect::<Vec<_>>();
        assert_eq!(Value::Array(references), expected[label]["references"]);
    }
}

#[test]
fn incomplete_diagnostic_fixture_suppresses_uncertain_rules() {
    let source = include_str!("fixtures/diagnostics/incomplete-call.stan");
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/diagnostics/incomplete-call.expected.json"
    ))
    .unwrap();
    let analysis = analyze_revision(source, Revision::default());
    let diagnostics = lint(&analysis, &LintConfig::default());
    for code in expected["absent_codes"].as_array().unwrap() {
        let code = code.as_str().unwrap();
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.0 == code)
        );
    }
}

#[test]
fn formatting_fixture_is_exact_and_idempotent() {
    let source = include_str!("fixtures/formatting/basic.stan");
    let expected = include_str!("fixtures/formatting/basic.expected.stan");
    let formatted = format(
        &analyze_revision(source, Revision::default()),
        &FormatterConfig::default(),
    )
    .unwrap();
    assert_eq!(formatted, expected);
    assert_eq!(
        format(
            &analyze_revision(&formatted, Revision::default()),
            &FormatterConfig::default()
        )
        .unwrap(),
        formatted
    );
}

#[test]
fn malformed_fixture_remains_lossless_and_validates_invariants() {
    let source = include_str!("fixtures/malformed/unclosed.stan");
    let analysis = analyze_revision(source, Revision::default());
    assert_eq!(analysis.syntax.reconstruct(), source);
    assert_eq!(analysis.semantics.validate(), Ok(()));
}
