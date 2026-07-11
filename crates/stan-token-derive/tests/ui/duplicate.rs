use stan_token_derive::Token;

#[derive(Token)]
enum Duplicate {
    #[token("same")]
    First,
    #[token("same")]
    Second,
}

fn main() {}

