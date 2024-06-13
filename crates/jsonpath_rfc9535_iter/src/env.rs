use std::collections::HashMap;

use crate::{
    errors::JSONPathError,
    function::FunctionRegister,
    iter::QueryIter,
    standard_functions::{Count, Length, Match, Search, Value},
    Query,
};

#[derive(Debug)]
pub struct Environment {
    pub function_register: FunctionRegister,
}

impl Environment {
    pub fn new() -> Self {
        let mut function_register: FunctionRegister = HashMap::new();
        function_register.insert("count".to_string(), Box::new(Count::new()));
        function_register.insert("length".to_string(), Box::new(Length::new()));
        function_register.insert("match".to_string(), Box::new(Match::new()));
        function_register.insert("search".to_string(), Box::new(Search::new()));
        function_register.insert("value".to_string(), Box::new(Value::new()));

        Self { function_register }
    }

    pub fn find<'v>(
        &'static self,
        expr: &str,
        value: &'v serde_json::Value,
    ) -> Result<QueryIter<'v>, JSONPathError> {
        let query = Query::standard(expr)?;
        Ok(QueryIter::new(self, value, query))
    }
}
