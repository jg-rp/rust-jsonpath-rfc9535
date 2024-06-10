use std::{collections::HashMap, rc::Rc};

use crate::{
    errors::JSONPathError,
    function::FunctionRegister,
    iter::QueryIter,
    standard_functions::{Count, Length, Match, Search, Value},
    Query,
};

#[derive(Debug, Clone)]
pub struct Environment {
    pub function_register: FunctionRegister,
}

impl Environment {
    pub fn new() -> Self {
        let mut function_register: FunctionRegister = HashMap::new();
        function_register.insert("count".to_string(), Rc::new(Count::new()));
        function_register.insert("length".to_string(), Rc::new(Length::new()));
        function_register.insert("match".to_string(), Rc::new(Match::new()));
        function_register.insert("search".to_string(), Rc::new(Search::new()));
        function_register.insert("value".to_string(), Rc::new(Value::new()));

        Self { function_register }
    }

    pub fn find<'v>(
        &self,
        expr: &str,
        value: &'v serde_json::Value,
    ) -> Result<QueryIter<'v>, JSONPathError> {
        let query = Query::standard(expr)?;
        Ok(QueryIter::new(Rc::new(self.clone()), value, query))
    }
}
