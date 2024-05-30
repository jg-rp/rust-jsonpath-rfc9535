use serde_json::Value;

use crate::{ast::NodeList, errors::JSONPathError, Query};

pub fn find<'a>(expr: &str, value: &'a Value) -> Result<NodeList<'a>, JSONPathError> {
    let query = Query::standard(expr)?;
    query.find(value)
}
