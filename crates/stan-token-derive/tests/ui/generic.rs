use stan_token_derive::Token;

#[derive(Token)]
enum Generic<T> {
    #[token("value")]
    Value,
}

fn main() {}

