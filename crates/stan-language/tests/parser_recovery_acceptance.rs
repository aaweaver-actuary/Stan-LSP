use stan_language::{ParseResult, SyntaxKind, SyntaxNodeKind, lex, parse};

fn parse_source(source: &str) -> ParseResult {
    let lexed = lex(source);
    parse(source.into(), lexed.tokens.into())
}

fn assert_lossless_and_bounded(source: &str, result: &ParseResult) {
    assert_eq!(result.tree.reconstruct(), source);

    let source_len = u32::try_from(source.len()).unwrap();
    let mut previous_token_end = 0;
    for token in result.tree.tokens() {
        assert!(token.range.start <= token.range.end);
        assert!(token.range.end <= source_len);
        assert!(token.range.start >= previous_token_end);
        previous_token_end = token.range.end;
    }

    for (index, node) in result.tree.nodes().iter().enumerate() {
        assert!(node.range.start <= node.range.end, "node {index}");
        assert!(node.range.end <= source_len, "node {index}");
        assert!(
            node.token_range.start <= node.token_range.end,
            "node {index}"
        );
        assert!(
            node.token_range.end <= result.tree.tokens().len(),
            "node {index}"
        );
        if let Some(parent) = node.parent {
            let parent = &result.tree.nodes()[parent];
            assert!(parent.range.start <= node.range.start, "node {index}");
            assert!(node.range.end <= parent.range.end, "node {index}");
        }
    }

    assert_eq!(
        result.tree.tokens().last().map(|token| token.kind),
        Some(SyntaxKind::EndOfFile)
    );
}

#[test]
fn unterminated_parameter_list_stops_before_function_body() {
    let source = include_str!("fixtures/parser/recovery/function-parameter-boundary.stan");
    let result = parse_source(source);
    assert_lossless_and_bounded(source, &result);

    let function = result
        .tree
        .source_file()
        .function_declarations()
        .next()
        .expect("the incomplete function should remain visible");
    let parameters = function.parameters();
    assert_eq!(parameters.len(), 1);

    let parameter = parameters[0];
    assert_eq!(parameter.name().text(), "x");
    assert_eq!(parameter.type_syntax().text().trim(), "real");
    let range = parameter.range();
    assert_eq!(
        &source[range.start as usize..range.end as usize],
        "real x",
        "body tokens must not be included in the recovered parameter range"
    );
    assert!(function.body_range().is_some());
}

#[test]
fn incomplete_initializer_call_remains_a_variable_declaration() {
    let source = include_str!("fixtures/parser/recovery/incomplete-initializer-call.stan");
    let first = parse_source(source);
    let second = parse_source(source);
    assert_lossless_and_bounded(source, &first);
    assert_lossless_and_bounded(source, &second);

    let variables = first
        .tree
        .source_file()
        .variable_declarations()
        .collect::<Vec<_>>();
    assert_eq!(variables.len(), 1);
    assert_eq!(variables[0].type_syntax().unwrap().text().trim(), "real");
    let declarators = variables[0].declarators();
    assert_eq!(declarators.len(), 1);
    assert_eq!(declarators[0].name().text(), "y");

    assert_eq!(first.tree.source_file().function_declarations().count(), 0);
    let calls = first
        .tree
        .source_file()
        .call_expressions()
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].callee().unwrap().text(), "normal");
    assert!(!calls[0].is_complete());

    let first_nodes = first
        .tree
        .nodes()
        .iter()
        .map(|node| (node.kind, node.range, node.token_range.clone()))
        .collect::<Vec<_>>();
    let second_nodes = second
        .tree
        .nodes()
        .iter()
        .map(|node| (node.kind, node.range, node.token_range.clone()))
        .collect::<Vec<_>>();
    assert_eq!(first_nodes, second_nodes);
}

#[test]
fn incomplete_declaration_shapes_recover_type_and_name() {
    let cases = [
        (
            include_str!("fixtures/parser/recovery/declaration-scalar.stan"),
            "real",
            "x",
        ),
        (
            include_str!("fixtures/parser/recovery/declaration-vector.stan"),
            "vector[N]",
            "beta",
        ),
        (
            include_str!("fixtures/parser/recovery/declaration-array.stan"),
            "array[N] real",
            "theta",
        ),
    ];

    for (source, expected_type, expected_name) in cases {
        let result = parse_source(source);
        assert_lossless_and_bounded(source, &result);

        let variables = result
            .tree
            .source_file()
            .variable_declarations()
            .collect::<Vec<_>>();
        assert_eq!(variables.len(), 1, "{source:?}");
        assert_eq!(
            variables[0].type_syntax().unwrap().text().trim(),
            expected_type,
            "{source:?}"
        );
        let declarators = variables[0].declarators();
        assert_eq!(declarators.len(), 1, "{source:?}");
        assert_eq!(declarators[0].name().text(), expected_name, "{source:?}");
    }
}

#[test]
fn constraint_assignments_do_not_hide_declaration_initializers() {
    let source = include_str!("fixtures/parser/expressions/constraint-initializer.stan");
    let result = parse_source(source);
    assert_lossless_and_bounded(source, &result);

    assert_eq!(result.tree.source_file().variable_declarations().count(), 2);
    assert_eq!(
        result
            .tree
            .nodes()
            .iter()
            .filter(|node| node.kind == SyntaxNodeKind::Expression)
            .count(),
        2
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.0 == "syntax.expected-expression")
    );
}

#[test]
fn comparisons_before_later_assignment_or_sampling_are_not_constraint_depth() {
    let source = include_str!("fixtures/parser/recovery/comparison-before-assignment.stan");
    let result = parse_source(source);
    assert_lossless_and_bounded(source, &result);

    assert!(
        result
            .tree
            .nodes()
            .iter()
            .filter(|node| node.kind == SyntaxNodeKind::Expression)
            .count()
            >= 3
    );
    let sampling = result
        .tree
        .source_file()
        .sampling_statements()
        .next()
        .expect("sampling after an indexed comparison should remain visible");
    assert_eq!(sampling.distribution().unwrap().text(), "normal");
    assert!(sampling.is_complete());
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.0 == "syntax.expected-expression")
    );
}

#[test]
fn acceptance_fixtures_use_lf_line_endings() {
    for source in [
        include_str!("fixtures/parser/recovery/function-parameter-boundary.stan"),
        include_str!("fixtures/parser/recovery/incomplete-initializer-call.stan"),
        include_str!("fixtures/parser/recovery/declaration-scalar.stan"),
        include_str!("fixtures/parser/recovery/declaration-vector.stan"),
        include_str!("fixtures/parser/recovery/declaration-array.stan"),
        include_str!("fixtures/parser/expressions/constraint-initializer.stan"),
        include_str!("fixtures/parser/recovery/comparison-before-assignment.stan"),
    ] {
        assert!(!source.as_bytes().contains(&b'\r'));
    }
}
