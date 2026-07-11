use stan_token_derive::Token;

#[derive(Token)]
enum Malformed {
    #[token(value = "value")]
    Value,
}

fn main() {}

