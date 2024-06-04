use crate::{ast::NodeList, env::Environment, errors::JSONPathError, Query};
use lazy_static::lazy_static;
use serde_json::Value;

lazy_static! {
    static ref ENV: Environment = Environment::new();
}

pub fn find<'a>(expr: &str, value: &'a Value) -> Result<NodeList<'a>, JSONPathError> {
    let query = Query::standard(expr)?;
    query.find(value, &ENV)
}

// Same as `find`, but uses explicit `for` loops and vectors to collect intermediate nodes.
pub fn find_loop<'a>(expr: &str, value: &'a Value) -> Result<NodeList<'a>, JSONPathError> {
    let query = Query::standard(expr)?;
    query.find_loop(value, &ENV)
}
