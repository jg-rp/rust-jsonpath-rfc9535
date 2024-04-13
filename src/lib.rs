//! This crate is a JSONPath expression parser, producing a JSON implementation-
//! agnostic abstract syntax tree, following the JSONPath model described in
//! RFC 9535.
//!
//!
//!
pub mod env;
pub mod errors;
mod lexer;
mod parser;
pub mod query;
mod token;

pub use query::Query;

// TODO: convenience function for creating a new Parser with function extension
// signatures passed as arguments.

// TODO: convenience function for creating a new standard Parser.
