use serde_json::{Value, json};
use stan_language::{SyntaxKind, SyntaxNodeKind, TextRange, lex, parse};

fn range(range: TextRange) -> Value {
    json!([range.start, range.end])
}

fn snapshot(source: &str) -> Value {
    let lexed = lex(source);
    let result = parse(source.into(), lexed.tokens.into());
    let file = result.tree.source_file();

    assert_eq!(result.tree.reconstruct(), source);
    assert_tree_invariants(source, &result.tree);

    let mut output = json!({
        "blocks": file.program_blocks().map(|block| json!({
            "kind": format!("{:?}", block.kind()),
            "range": range(block.range()),
        })).collect::<Vec<_>>(),
        "functions": file.function_declarations().map(|function| json!({
            "range": range(function.range()),
            "name": function.name().map(|name| json!({"text": name.text(), "range": range(name.range())})),
            "return_type": function.return_type().map(|kind| json!({"text": kind.text(), "range": range(kind.range())})),
            "parameters": function.parameters().into_iter().map(|parameter| json!({
                "range": range(parameter.range()),
                "name": {"text": parameter.name().text(), "range": range(parameter.name().range())},
                "type": {"text": parameter.type_syntax().text(), "range": range(parameter.type_syntax().range())},
                "data_only": parameter.is_data_only(),
            })).collect::<Vec<_>>(),
            "body": function.body_range().map(range),
        })).collect::<Vec<_>>(),
        "variables": file.variable_declarations().map(|declaration| json!({
            "range": range(declaration.range()),
            "type": declaration.type_syntax().map(|kind| json!({"text": kind.text(), "range": range(kind.range())})),
            "declarators": declaration.declarators().into_iter().map(|declarator| json!({
                "range": range(declarator.range()),
                "name": {"text": declarator.name().text(), "range": range(declarator.name().range())},
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "loops": file.for_statements().map(|statement| json!({
            "range": range(statement.range()),
            "binder": statement.binder().map(|name| json!({"text": name.text(), "range": range(name.range())})),
            "body": statement.body_range().map(range),
        })).collect::<Vec<_>>(),
        "compounds": file.compound_statements().map(|statement| range(statement.range())).collect::<Vec<_>>(),
        "calls": file.call_expressions().map(|call| json!({
            "range": range(call.range()),
            "callee": call.callee().map(|name| json!({"text": name.text(), "range": range(name.range())})),
            "closing_paren": call.closing_paren().map(range),
            "complete": call.is_complete(),
            "declaration": call.is_declaration(),
        })).collect::<Vec<_>>(),
        "sampling": file.sampling_statements().map(|statement| json!({
            "range": range(statement.range()),
            "distribution": statement.distribution().map(|name| json!({"text": name.text(), "range": range(name.range())})),
            "closing_paren": statement.closing_paren().map(range),
            "complete": statement.is_complete(),
        })).collect::<Vec<_>>(),
        "diagnostics": result.diagnostics.iter().map(|diagnostic| json!({
            "code": diagnostic.code.0,
            "range": range(diagnostic.primary_range),
        })).collect::<Vec<_>>(),
    });
    output
        .as_object_mut()
        .unwrap()
        .retain(|_, value| !value.as_array().is_some_and(Vec::is_empty));
    output
}

fn assert_tree_invariants(source: &str, tree: &stan_language::SyntaxTree) {
    let source_len = u32::try_from(source.len()).unwrap();
    let mut previous_token_end = 0;
    for token in tree.tokens() {
        assert!(token.range.start <= token.range.end);
        assert!(token.range.end <= source_len);
        assert!(token.range.start >= previous_token_end);
        previous_token_end = token.range.end;
    }
    for (index, node) in tree.nodes().iter().enumerate() {
        assert!(node.range.start <= node.range.end, "node {index}");
        assert!(node.range.end <= source_len, "node {index}");
        assert!(
            node.token_range.start <= node.token_range.end,
            "node {index}"
        );
        assert!(node.token_range.end <= tree.tokens().len(), "node {index}");
        if let Some(parent) = node.parent {
            let parent = &tree.nodes()[parent];
            assert!(parent.range.start <= node.range.start, "node {index}");
            assert!(node.range.end <= parent.range.end, "node {index}");
        }
    }
    for kind in [
        SyntaxNodeKind::FunctionDeclaration,
        SyntaxNodeKind::VariableDeclaration,
        SyntaxNodeKind::ForStatement,
        SyntaxNodeKind::CompoundStatement,
        SyntaxNodeKind::FunctionCall,
        SyntaxNodeKind::SamplingStatement,
    ] {
        let ranges = tree
            .nodes()
            .iter()
            .filter(|node| node.kind == kind)
            .map(|node| node.range.start)
            .collect::<Vec<_>>();
        assert!(ranges.windows(2).all(|pair| pair[0] <= pair[1]), "{kind:?}");
    }
    assert_eq!(
        tree.tokens().last().map(|token| token.kind),
        Some(SyntaxKind::EndOfFile)
    );
}

macro_rules! fixture {
    ($name:ident, $source:literal, $expected:literal) => {
        #[test]
        fn $name() {
            let source = include_str!($source);
            let expected: Value = serde_json::from_str(include_str!($expected)).unwrap();
            assert_eq!(snapshot(source), expected);
        }
    };
}

fixture!(
    source_and_blocks,
    "fixtures/parser/program-blocks/valid.stan",
    "fixtures/parser/program-blocks/valid.expected.json"
);
fixture!(
    function_declarations,
    "fixtures/parser/function-declarations/valid.stan",
    "fixtures/parser/function-declarations/valid.expected.json"
);
fixture!(
    variable_declarations,
    "fixtures/parser/variable-declarations/valid.stan",
    "fixtures/parser/variable-declarations/valid.expected.json"
);
fixture!(
    loops_calls_and_sampling,
    "fixtures/parser/expressions/valid.stan",
    "fixtures/parser/expressions/valid.expected.json"
);
fixture!(
    incomplete_function_recovery,
    "fixtures/parser/recovery/function.stan",
    "fixtures/parser/recovery/function.expected.json"
);
fixture!(
    incomplete_declaration_recovery,
    "fixtures/parser/recovery/declaration.stan",
    "fixtures/parser/recovery/declaration.expected.json"
);
fixture!(
    incomplete_loop_recovery,
    "fixtures/parser/recovery/loop.stan",
    "fixtures/parser/recovery/loop.expected.json"
);
fixture!(
    incomplete_sampling_recovery,
    "fixtures/parser/recovery/sampling.stan",
    "fixtures/parser/recovery/sampling.expected.json"
);
fixture!(
    unicode_comments_and_spacing,
    "fixtures/parser/unicode/comments.stan",
    "fixtures/parser/unicode/comments.expected.json"
);

#[test]
fn exact_parser_fixtures_use_lf_line_endings() {
    for source in [
        include_str!("fixtures/parser/program-blocks/valid.stan"),
        include_str!("fixtures/parser/function-declarations/valid.stan"),
        include_str!("fixtures/parser/variable-declarations/valid.stan"),
        include_str!("fixtures/parser/expressions/valid.stan"),
        include_str!("fixtures/parser/recovery/function.stan"),
        include_str!("fixtures/parser/recovery/declaration.stan"),
        include_str!("fixtures/parser/recovery/loop.stan"),
        include_str!("fixtures/parser/recovery/sampling.stan"),
        include_str!("fixtures/parser/unicode/comments.stan"),
    ] {
        assert!(!source.as_bytes().contains(&b'\r'));
    }
}

#[test]
fn deterministic_malformed_inputs_always_progress_and_reconstruct() {
    let cases = [
        "functions { real f(real",
        "parameters { vector[",
        "model { y ~ normal(",
        "model { for (n in",
        "generated quantities { real y =",
        "model { (((((((((((((((((((((((((((((",
        "model { ,,,,,;;;;; }",
        "model { /* comment between */ for /* x */ (n in",
    ];
    for source in cases {
        let first = snapshot(source);
        let second = snapshot(source);
        assert_eq!(first, second, "{source:?}");
    }
}
