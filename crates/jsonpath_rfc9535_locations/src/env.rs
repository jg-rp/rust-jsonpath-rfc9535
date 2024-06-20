use std::collections::HashMap;

use crate::{
    errors::JSONPathError,
    function::FunctionRegister,
    node::NodeList,
    standard_functions::{Count, Length, Match, Search, Value},
    Query,
};

pub struct Environment {
    pub function_register: FunctionRegister,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
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
        &'static self,
        expr: &str,
        value: &'a serde_json::Value,
    ) -> Result<NodeList<'a>, JSONPathError> {
        let query = Query::standard(expr)?;
        Ok(query.find(value, self))
    }
}
