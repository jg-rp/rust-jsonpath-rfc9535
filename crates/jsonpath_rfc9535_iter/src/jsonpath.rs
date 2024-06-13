use lazy_static::lazy_static;
use serde_json::Value;

use crate::{env::Environment, errors::JSONPathError, iter::QueryIter, Query};

lazy_static! {
    static ref ENV: Environment = Environment::new();
}

pub fn find<'a, 'v>(expr: &str, value: &'v Value) -> Result<QueryIter<'v>, JSONPathError> {
    let query = Query::standard(expr)?;
    Ok(QueryIter::new(&ENV, value, query))
}
