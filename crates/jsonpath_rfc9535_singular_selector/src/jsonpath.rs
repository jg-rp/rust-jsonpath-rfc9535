use crate::{env::Environment, errors::JSONPathError, node::NodeList, Query};
use lazy_static::lazy_static;
use serde_json::Value;

lazy_static! {
    pub static ref ENV: Environment = Environment::new();
}

pub fn find<'a>(expr: &str, value: &'a Value) -> Result<NodeList<'a>, JSONPathError> {
    let query = Query::standard(expr)?;
    Ok(query.find(value, &ENV))
}
