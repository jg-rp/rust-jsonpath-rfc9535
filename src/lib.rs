//! A JSONPath expression parser, producing a JSON implementation-agnostic
//! abstract syntax tree, following the JSONPath model described in RFC 9535.
//!
//!
//!
pub mod env;
pub mod errors;
mod lexer;
pub mod parser;
pub mod query;
mod token;

pub use env::Env;
pub use parser::Parser;
pub use query::Query;
