use crate::{env::Environment, errors::JSONPathError, iter::QueryIter};
use serde_json::Value;

pub fn find<'a, 'v>(expr: &str, value: &'v Value) -> Result<QueryIter<'v>, JSONPathError> {
    Environment::new().find(expr, value)
}
