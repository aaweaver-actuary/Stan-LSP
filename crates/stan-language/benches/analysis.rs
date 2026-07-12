use std::{hint::black_box, time::Instant};

use stan_language::{Revision, analyze_revision};

fn main() {
    let model = include_str!("../tests/fixtures/parser/valid/basic.stan");
    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(analyze_revision(black_box(model), Revision::default()));
    }
    let elapsed = start.elapsed();
    println!(
        "complete analysis: {:?} per iteration ({iterations} iterations)",
        elapsed / iterations
    );
}
