use std::fmt;

use serde_json::Value;

use crate::{env::Environment, function::ExpressionType, node::NodeList, Query};

#[derive(Debug)]
pub enum FilterExpression {
    True,
    False,
    Null,
    String {
        value: String,
    },
    Int {
        value: i64,
    },
    Float {
        value: f64,
    },
    Not {
        expression: Box<FilterExpression>,
    },
    Logical {
        left: Box<FilterExpression>,
        operator: LogicalOperator,
        right: Box<FilterExpression>,
    },
    Comparison {
        left: Box<FilterExpression>,
        operator: ComparisonOperator,
        right: Box<FilterExpression>,
    },
    RelativeQuery {
        query: Box<Query>,
    },
    RootQuery {
        query: Box<Query>,
    },
    Function {
        name: String,
        args: Vec<FilterExpression>,
    },
}

impl FilterExpression {
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            FilterExpression::True { .. }
                | FilterExpression::False { .. }
                | FilterExpression::Null { .. }
                | FilterExpression::String { .. }
                | FilterExpression::Int { .. }
                | FilterExpression::Float { .. }
        )
    }
}

impl FilterExpression {
    pub fn evaluate<'v>(
        &self,
        env: &'static Environment,
        root: &'v Value,
        current: &'v Value,
    ) -> FilterExpressionResult<'v> {
        match self {
            FilterExpression::True => FilterExpressionResult::Bool(true),
            FilterExpression::False => FilterExpressionResult::Bool(false),
            FilterExpression::Null => FilterExpressionResult::Null,
            FilterExpression::String { value } => FilterExpressionResult::String(value.to_owned()),
            FilterExpression::Int { value } => FilterExpressionResult::Int(*value),
            FilterExpression::Float { value } => FilterExpressionResult::Float(*value),
            FilterExpression::Not { expression } => {
                if !is_truthy(expression.evaluate(env, root, current)) {
                    FilterExpressionResult::Bool(true)
                } else {
                    FilterExpressionResult::Bool(false)
                }
            }
            FilterExpression::Logical {
                left,
                operator,
                right,
            } => {
                if logical(
                    left.evaluate(env, root, current),
                    operator,
                    right.evaluate(env, root, current),
                ) {
                    FilterExpressionResult::Bool(true)
                } else {
                    FilterExpressionResult::Bool(false)
                }
            }
            FilterExpression::Comparison {
                left,
                operator,
                right,
            } => {
                if compare(
                    left.evaluate(env, root, current),
                    operator,
                    right.evaluate(env, root, current),
                ) {
                    FilterExpressionResult::Bool(true)
                } else {
                    FilterExpressionResult::Bool(false)
                }
            }
            FilterExpression::RelativeQuery { query } => {
                FilterExpressionResult::Nodes(query.find(current, env))
            }
            FilterExpression::RootQuery { query } => {
                FilterExpressionResult::Nodes(query.find(root, env))
            }
            FilterExpression::Function { name, args } => {
                let fn_ext = env
                    .function_register
                    .get(name)
                    .unwrap_or_else(|| panic!("unknown function '{}'", name));

                let _args = args
                    .iter()
                    .map(|expr| expr.evaluate(env, root, current))
                    .enumerate()
                    .map(|(i, rv)| unpack_result(rv, &fn_ext.sig().param_types, i))
                    .collect();

                fn_ext.call(_args)
            }
        }
    }
}

impl fmt::Display for FilterExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterExpression::True { .. } => f.write_str("true"),
            FilterExpression::False { .. } => f.write_str("false"),
            FilterExpression::Null { .. } => f.write_str("null"),
            FilterExpression::String { value, .. } => write!(f, "'{value}'"),
            FilterExpression::Int { value, .. } => write!(f, "{value}"),
            FilterExpression::Float { value, .. } => write!(f, "{value}"),
            FilterExpression::Not { expression, .. } => write!(f, "!{expression}"),
            FilterExpression::Logical {
                left,
                operator,
                right,
                ..
            } => write!(f, "({left} {operator} {right})"),
            FilterExpression::Comparison {
                left,
                operator,
                right,
                ..
            } => write!(f, "{left} {operator} {right}"),
            FilterExpression::RelativeQuery { query, .. } => {
                write!(
                    f,
                    "@{}",
                    query
                        .segments
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join("")
                )
            }
            FilterExpression::RootQuery { query, .. } => {
                write!(
                    f,
                    "${}",
                    query
                        .segments
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join("")
                )
            }
            FilterExpression::Function { name, args, .. } => {
                write!(
                    f,
                    "{}({})",
                    name,
                    args.iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        }
    }
}

#[derive(Debug)]
pub enum LogicalOperator {
    And,
    Or,
}

impl fmt::Display for LogicalOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicalOperator::And => f.write_str("&&"),
            LogicalOperator::Or => f.write_str("||"),
        }
    }
}

#[derive(Debug)]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Ge,
    Gt,
    Le,
    Lt,
}

impl fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComparisonOperator::Eq => f.write_str("=="),
            ComparisonOperator::Ne => f.write_str("!="),
            ComparisonOperator::Ge => f.write_str(">="),
            ComparisonOperator::Gt => f.write_str(">"),
            ComparisonOperator::Le => f.write_str("<="),
            ComparisonOperator::Lt => f.write_str("<"),
        }
    }
}

#[derive(Debug)]
pub enum FilterExpressionResult<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    Null,
    String(String),
    Array(&'a Value),
    Object(&'a Value),
    Nodes(NodeList<'a>),
    Nothing,
}

impl<'a> FilterExpressionResult<'a> {
    pub fn from_json_value(value: &'a Value) -> Self {
        match value {
            Value::Bool(v) => FilterExpressionResult::Bool(*v),
            Value::Null => FilterExpressionResult::Null,
            Value::Number(n) => {
                if n.is_f64() {
                    FilterExpressionResult::Float(n.as_f64().unwrap())
                } else {
                    FilterExpressionResult::Int(n.as_i64().unwrap())
                }
            }
            Value::String(s) => FilterExpressionResult::String(s.to_owned()),
            Value::Array(_) => FilterExpressionResult::Array(value),
            Value::Object(_) => FilterExpressionResult::Object(value),
        }
    }
}

pub fn is_truthy(rv: FilterExpressionResult) -> bool {
    match rv {
        FilterExpressionResult::Nothing => false,
        FilterExpressionResult::Nodes(nodes) => !nodes.is_empty(),
        FilterExpressionResult::Bool(v) => v,
        _ => true,
    }
}

pub fn is_truthy_ref(rv: &FilterExpressionResult) -> bool {
    match rv {
        FilterExpressionResult::Nothing => false,
        FilterExpressionResult::Nodes(nodes) => !nodes.is_empty(),
        FilterExpressionResult::Bool(v) => *v,
        _ => true,
    }
}

pub fn logical(
    left: FilterExpressionResult,
    op: &LogicalOperator,
    right: FilterExpressionResult,
) -> bool {
    match op {
        LogicalOperator::And => is_truthy(left) && is_truthy(right),
        LogicalOperator::Or => is_truthy(left) || is_truthy(right),
    }
}

fn nodes_or_singular(rv: FilterExpressionResult<'_>) -> FilterExpressionResult<'_> {
    match rv {
        FilterExpressionResult::Nodes(ref nodes) => {
            if nodes.len() == 1 {
                FilterExpressionResult::from_json_value(nodes.first().unwrap().value)
            } else {
                rv
            }
        }
        _ => rv,
    }
}

pub fn compare(
    left: FilterExpressionResult,
    op: &ComparisonOperator,
    right: FilterExpressionResult,
) -> bool {
    use ComparisonOperator::*;
    let left = nodes_or_singular(left);
    let right = nodes_or_singular(right);
    match op {
        Eq => eq(&left, &right),
        Ne => !eq(&left, &right),
        Lt => lt(&left, &right),
        Gt => lt(&right, &left),
        Ge => lt(&right, &left) || eq(&left, &right),
        Le => lt(&left, &right) || eq(&left, &right),
    }
}

fn eq(left: &FilterExpressionResult, right: &FilterExpressionResult) -> bool {
    use FilterExpressionResult::*;
    match (left, right) {
        (Nothing, Nothing) => true,
        (Nodes(nodes), Nothing) | (Nothing, Nodes(nodes)) => nodes.is_empty(),
        (Nothing, _) | (_, Nothing) => false,
        (Nodes(left), Nodes(right)) => {
            if left.is_empty() && right.is_empty() {
                true
            } else {
                // Only singular queries can be compared
                unreachable!()
            }
        }
        (FilterExpressionResult::Int(l), FilterExpressionResult::Int(r)) => l == r,
        (FilterExpressionResult::Float(l), FilterExpressionResult::Float(r)) => l == r,
        (FilterExpressionResult::Int(l), FilterExpressionResult::Float(r)) => *l as f64 == *r,
        (FilterExpressionResult::Float(l), FilterExpressionResult::Int(r)) => *l == *r as f64,
        (FilterExpressionResult::Null, FilterExpressionResult::Null) => true,
        (FilterExpressionResult::Bool(l), FilterExpressionResult::Bool(r)) => l == r,
        (FilterExpressionResult::String(l), FilterExpressionResult::String(r)) => l == r,
        (FilterExpressionResult::Array(l), FilterExpressionResult::Array(r)) => *l == *r,
        (FilterExpressionResult::Object(l), FilterExpressionResult::Object(r)) => *l == *r,
        _ => false,
    }
}

fn lt(left: &FilterExpressionResult, right: &FilterExpressionResult) -> bool {
    match (left, right) {
        (FilterExpressionResult::String(l), FilterExpressionResult::String(r)) => l < r,
        (FilterExpressionResult::Bool(_), FilterExpressionResult::Bool(_)) => false,
        (FilterExpressionResult::Int(l), FilterExpressionResult::Int(r)) => l < r,
        (FilterExpressionResult::Float(l), FilterExpressionResult::Float(r)) => l < r,
        (FilterExpressionResult::Int(l), FilterExpressionResult::Float(r)) => (*l as f64) < *r,
        (FilterExpressionResult::Float(l), FilterExpressionResult::Int(r)) => *l < *r as f64,
        _ => false,
    }
}

pub fn unpack_result<'a>(
    rv: FilterExpressionResult<'a>,
    param_types: &[ExpressionType],
    index: usize,
) -> FilterExpressionResult<'a> {
    if matches!(param_types.get(index).unwrap(), ExpressionType::Nodes) {
        return rv;
    }

    match &rv {
        FilterExpressionResult::Nodes(nodes) => match nodes.len() {
            0 => FilterExpressionResult::Nothing,
            1 => FilterExpressionResult::from_json_value(nodes.first().unwrap().value),
            _ => rv,
        },
        _ => rv,
    }
}
