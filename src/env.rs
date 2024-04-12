use std::{collections::HashMap, ops::RangeInclusive};

pub enum ExpressionType {
    Logical,
    Nodes,
    Value,
}

pub struct FunctionSignature {
    pub param_types: Vec<ExpressionType>,
    pub return_type: ExpressionType,
}

pub struct Env {
    pub index_range: RangeInclusive<i64>,
    pub functions: HashMap<String, FunctionSignature>,
}

impl Env {
    pub fn standard() -> Self {
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

        Env {
            index_range: ((-2_i64).pow(53) + 1..=2_i64.pow(53) - 1),
            functions,
        }
    }
}
