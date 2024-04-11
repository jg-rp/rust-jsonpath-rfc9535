#![feature(test)]

extern crate test;
pub mod env;
pub mod errors;
pub mod lexer;
pub mod parser;
pub mod query;
pub mod token;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_add_two(b: &mut Bencher) {
        b.iter(|| {
            // Some comment
            add(2, 2)
        });
    }
}
