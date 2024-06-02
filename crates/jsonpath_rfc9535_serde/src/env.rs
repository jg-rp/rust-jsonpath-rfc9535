use std::collections::HashMap;

use crate::{
    ast::NodeList,
    errors::JSONPathError,
    function::FunctionRegister,
    standard_functions::{Count, Length, Match, Search, Value},
    Query,
};

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

    pub fn find<'a>(
        &self,
        expr: &str,
        value: &'a serde_json::Value,
    ) -> Result<NodeList<'a>, JSONPathError> {
        let query = Query::standard(expr)?;
        query.find(value, &self)
    }
}
