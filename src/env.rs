use std::collections::HashMap;

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
    pub max_index: i64,
    pub min_index: i64,
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
            max_index: 2_i64.pow(53) - 1,
            min_index: (-2_i64).pow(53) + 1,
            functions,
        }
    }
}
