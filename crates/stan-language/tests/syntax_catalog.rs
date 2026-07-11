use std::collections::BTreeSet;

use stan_language::{Directive, Keyword, LegacyLanguageElement, Symbol};

const INVENTORY: &str = include_str!("../../../catalog/stan-2.39/language-tokens.txt");

fn assert_round_trip<T>(all: &[T], spelling: impl Fn(&T) -> &'static str)
where
    T: Copy + Ord + std::fmt::Debug + std::str::FromStr + PartialEq,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    let mut unique = BTreeSet::new();
    for value in all {
        let token = spelling(value);
        assert!(unique.insert(token), "duplicate token {token:?}");
        assert_eq!(token.parse::<T>().unwrap(), *value);
    }
}

#[test]
fn all_fixed_tokens_round_trip() {
    assert_round_trip(Keyword::ALL, Keyword::as_str);
    assert_round_trip(Symbol::ALL, Symbol::as_str);
    assert_round_trip(Directive::ALL, Directive::as_str);
    assert_round_trip(LegacyLanguageElement::ALL, LegacyLanguageElement::as_str);
}

#[test]
fn all_active_tokens_have_roles() {
    assert!(
        Keyword::ALL
            .iter()
            .all(|keyword| !keyword.roles().is_empty())
    );
    assert!(Symbol::ALL.iter().all(|symbol| !symbol.roles().is_empty()));
}

#[test]
fn enums_match_the_pinned_compiler_inventory() {
    let actual = Keyword::ALL
        .iter()
        .map(|value| ("keyword", value.as_str()))
        .chain(Symbol::ALL.iter().map(|value| ("symbol", value.as_str())))
        .chain(
            Directive::ALL
                .iter()
                .map(|value| ("directive", value.as_str())),
        )
        .collect::<BTreeSet<_>>();
    let expected = INVENTORY
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.split_once('\t')
                .expect("inventory entries contain a tab")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
}
