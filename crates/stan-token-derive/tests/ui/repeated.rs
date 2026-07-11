use stan_token_derive::Token;

#[derive(Token)]
enum Repeated {
    #[token("one")]
    #[token("two")]
    Value,
}

fn main() {}

