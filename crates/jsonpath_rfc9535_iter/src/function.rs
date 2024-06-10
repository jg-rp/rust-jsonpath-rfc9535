use std::{collections::HashMap, fmt::Debug, rc::Rc};

use crate::filter::FilterExpressionResult;

#[derive(Debug)]
pub enum ExpressionType {
    Logical,
    Nodes,
    Value,
}

pub struct FunctionSignature {
    pub param_types: Vec<ExpressionType>,
    pub return_type: ExpressionType,
}

pub fn standard_functions() -> HashMap<String, FunctionSignature> {
    let mut functions = HashMap::new();

    functions.insert(
        "count".to_owned(),
        FunctionSignature {
            param_types: vec![ExpressionType::Nodes],
            return_type: ExpressionType::Value,
        },
    );

    functions.insert(
        "length".to_owned(),
        FunctionSignature {
            param_types: vec![ExpressionType::Value],
            return_type: ExpressionType::Value,
        },
    );

    functions.insert(
        "match".to_owned(),
        FunctionSignature {
            param_types: vec![ExpressionType::Value, ExpressionType::Value],
            return_type: ExpressionType::Logical,
        },
    );

    functions.insert(
        "search".to_owned(),
        FunctionSignature {
            param_types: vec![ExpressionType::Value, ExpressionType::Value],
            return_type: ExpressionType::Logical,
        },
    );

    functions.insert(
        "value".to_owned(),
        FunctionSignature {
            param_types: vec![ExpressionType::Nodes],
            return_type: ExpressionType::Value,
        },
    );

    functions
}

pub trait FunctionExtension {
    fn call<'a>(&self, args: Vec<FilterExpressionResult<'a>>) -> FilterExpressionResult<'a>;
    fn sig(&self) -> FunctionSignature;
}

impl Debug for dyn FunctionExtension + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sig = self.sig();
        write!(f, "({:?}) -> {:?}", sig.param_types, sig.return_type)
    }
}

pub type FunctionRegister = HashMap<String, Rc<dyn FunctionExtension + Sync>>;
