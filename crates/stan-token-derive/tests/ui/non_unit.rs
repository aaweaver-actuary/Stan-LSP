use stan_token_derive::Token;

#[derive(Token)]
enum NonUnit {
    #[token("tuple")]
    Tuple(u8),
}

fn main() {}

