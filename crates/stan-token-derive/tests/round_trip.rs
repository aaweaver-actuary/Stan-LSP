use stan_token_derive::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Token)]
enum Greek {
    #[token("alpha")]
    Alpha,
    #[token("beta")]
    Beta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Token)]
enum Symbol {
    #[token("+")]
    Plus,
    #[token("<=")]
    LessThanOrEqual,
}

#[test]
fn every_generated_api_round_trips() {
    assert_eq!(Greek::ALL, &[Greek::Alpha, Greek::Beta]);
    for value in Greek::ALL {
        let spelling = value.as_str();
        assert_eq!(Greek::from_str(spelling).unwrap(), *value);
        assert_eq!(spelling.parse::<Greek>().unwrap(), *value);
        assert_eq!(Greek::try_from(spelling).unwrap(), *value);
        assert_eq!(value.as_ref(), spelling);
        assert_eq!(value.to_string(), spelling);
    }

    assert_eq!(Symbol::ALL, &[Symbol::Plus, Symbol::LessThanOrEqual]);
}

#[test]
fn errors_retain_the_input() {
    let error = Greek::from_str("Alpha").unwrap_err();
    assert_eq!(error.input(), "Alpha");
    assert_eq!(error.to_string(), "unknown Greek token \"Alpha\"");
}
