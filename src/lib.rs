//! A JSONPath expression parser, producing a JSON implementation-agnostic
//! abstract syntax tree, following the JSONPath model described in RFC 9535.
//!
//!
//!
pub mod errors;
pub mod lexer;
pub mod parser;
pub mod query;
mod token;

pub use parser::standard_functions;
pub use parser::ExpressionType;
pub use parser::FunctionSignature;
pub use parser::Parser;
pub use query::Query;
