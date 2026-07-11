use stan_token_derive::Token;

#[derive(Token)]
enum Empty {
    #[token("")]
    Value,
}

fn main() {}

