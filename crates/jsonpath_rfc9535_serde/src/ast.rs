//! Structs and enums that make up a JSONPath query syntax tree.
//!
//! A [`Query`] contains zero or more [`Segment`]s, and each segment contains one
//! or more [`Selector`]s. When a segment includes a _filter selector_, that
//! filter selector is a tree of [`FilterExpression`]s.
//!
//! [RFC 9535]: https://datatracker.ietf.org/doc/html/rfc9535

use itertools::Itertools;
use lazy_static::lazy_static;
use serde_json::Value;
use std::{
    cmp,
    fmt::{self, Write},
};

use crate::{errors::JSONPathError, parser::JSONPathParser};

lazy_static! {
    static ref PARSER: JSONPathParser = JSONPathParser::new();
}

#[derive(Debug, Clone)]
pub struct Node<'a> {
    value: &'a Value,
    location: String,
}

impl<'a> Node<'a> {
    fn new_child_member(&self, value: &'a Value, loc: &str) -> Self {
        Node {
            value,
            location: format!("{}['{}']", self.location, loc),
        }
    }

    fn new_child_element(&self, value: &'a Value, loc: usize) -> Self {
        Node {
            value,
            location: format!("{}[{}]", self.location, loc),
        }
    }
}

pub type NodeList<'a> = Vec<Node<'a>>;

struct QueryContext<'a> {
    root: &'a Value,
}

pub struct FilterContext<'a> {
    root: &'a Value,
    current: &'a Value,
}

pub enum FilterExpressionResult<'a> {
    True,
    False,
    NullLiteral,
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    Nodes(NodeList<'a>),
    Nothing,
}

#[derive(Debug)]
pub struct Query {
    pub segments: Vec<Segment>,
}

impl Query {
    pub fn new(segments: Vec<Segment>) -> Self {
        Query { segments }
    }

    pub fn standard(expr: &str) -> Result<Self, JSONPathError> {
        PARSER.parse(expr)
    }

    pub fn find<'a>(&self, value: &'a Value) -> Result<NodeList<'a>, JSONPathError> {
        let context = QueryContext { root: value };

        let root_node = Node {
            value,
            location: String::from("$"),
        };

        self.segments
            .iter()
            .try_fold(vec![root_node], |nodes, segment| {
                segment.resolve(nodes, &context)
            })
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn is_singular(&self) -> bool {
        self.segments.iter().all(|segment| {
            if let Segment::Child { selectors, .. } = segment {
                return selectors.len() == 1
                    && selectors.first().is_some_and(|selector| {
                        matches!(selector, Selector::Name { .. } | Selector::Index { .. })
                    });
            }
            false
        })
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "${}",
            self.segments
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

#[derive(Debug)]
pub enum Segment {
    Child { selectors: Vec<Selector> },
    Recursive { selectors: Vec<Selector> },
    Eoi,
}

impl Segment {
    fn resolve<'a>(
        &self,
        nodes: NodeList<'a>,
        context: &QueryContext,
    ) -> Result<NodeList<'a>, JSONPathError> {
        match self {
            Segment::Child { selectors } => {
                let child_nodes: Result<Vec<_>, _> = nodes
                    .iter()
                    .flat_map(|node| selectors.iter().map(|s| s.resolve(node, context)))
                    .collect();

                Ok(child_nodes?.into_iter().flatten().collect())
            }
            Segment::Recursive { selectors } => {
                let descendant_nodes: Result<Vec<_>, _> = nodes
                    .iter()
                    .flat_map(|n| visit(n))
                    .flat_map(|node| selectors.iter().map(move |s| s.resolve(&node, context)))
                    .collect();

                Ok(descendant_nodes?.into_iter().flatten().collect())
            }
            Segment::Eoi {} => Ok(nodes),
        }
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Segment::Child { selectors, .. } => {
                write!(
                    f,
                    "[{}]",
                    selectors
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Segment::Recursive { selectors, .. } => {
                write!(
                    f,
                    "..[{}]",
                    selectors
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Segment::Eoi => Ok(()),
        }
    }
}

#[derive(Debug)]
pub enum Selector {
    Name {
        name: String,
    },
    Index {
        index: i64,
    },
    Slice {
        start: Option<i64>,
        stop: Option<i64>,
        step: Option<i64>,
    },
    Wild,
    Filter {
        expression: Box<FilterExpression>,
    },
}

impl Selector {
    fn resolve<'a, 'b>(
        &self,
        node: &'b Node<'a>,
        context: &QueryContext,
    ) -> Result<NodeList<'a>, JSONPathError> {
        match self {
            Selector::Name { name } => {
                if let Some(v) = node.value.get(name) {
                    Ok(vec![node.new_child_member(v, name)])
                } else {
                    Ok(Vec::new())
                }
            }
            Selector::Index { index } => {
                if let Some(v) = node.value.get(*index as usize) {
                    Ok(vec![node.new_child_element(v, *index as usize)])
                } else {
                    Ok(Vec::new())
                }
            }
            Selector::Slice { start, stop, step } => {
                if let Some(array) = node.value.as_array() {
                    Ok(slice(array, *start, *stop, *step)
                        .into_iter()
                        .map(|(i, v)| node.new_child_element(v, i as usize))
                        .collect())
                } else {
                    Ok(Vec::new())
                }
            }
            Selector::Wild {} => match node.value {
                Value::Array(arr) => Ok(arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| node.new_child_element(v, i))
                    .collect()),
                Value::Object(obj) => Ok(obj
                    .iter()
                    .map(|(k, v)| node.new_child_member(v, k))
                    .collect()),
                _ => Ok(Vec::new()),
            },
            Selector::Filter { expression } => match node.value {
                Value::Array(arr) => arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        expression
                            .evaluate(&FilterContext {
                                root: context.root,
                                current: v,
                            })
                            .map(|r| (i, v, r))
                    })
                    .filter_ok(|(_, _, r)| is_truthy(r))
                    .map_ok(|(i, v, _)| node.new_child_element(v, i))
                    .collect(),
                Value::Object(obj) => obj
                    .iter()
                    .map(|(k, v)| {
                        expression
                            .evaluate(&FilterContext {
                                root: context.root,
                                current: v,
                            })
                            .map(|r| (k, v, r))
                    })
                    .filter_ok(|(_, _, r)| is_truthy(r))
                    .map_ok(|(k, v, _)| node.new_child_member(v, k))
                    .collect(),
                _ => Ok(Vec::new()),
            },
        }
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Selector::Name { name, .. } => write!(f, "'{name}'"),
            Selector::Index {
                index: array_index, ..
            } => write!(f, "{array_index}"),
            Selector::Slice {
                start, stop, step, ..
            } => {
                write!(
                    f,
                    "{}:{}:{}",
                    start
                        .and_then(|i| Some(i.to_string()))
                        .unwrap_or(String::from("")),
                    stop.and_then(|i| Some(i.to_string()))
                        .unwrap_or(String::from("")),
                    step.and_then(|i| Some(i.to_string()))
                        .unwrap_or(String::from("1")),
                )
            }
            Selector::Wild { .. } => f.write_char('*'),
            Selector::Filter { expression, .. } => write!(f, "?{expression}"),
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
    fn evaluate<'a>(
        &self,
        context: &FilterContext<'a>,
    ) -> Result<FilterExpressionResult<'a>, JSONPathError> {
        match self {
            FilterExpression::True => Ok(FilterExpressionResult::True),
            FilterExpression::False => Ok(FilterExpressionResult::False),
            FilterExpression::Null => Ok(FilterExpressionResult::NullLiteral),
            FilterExpression::String { value } => {
                Ok(FilterExpressionResult::StringLiteral(value.clone()))
            }
            FilterExpression::Int { value } => {
                Ok(FilterExpressionResult::IntLiteral(value.clone()))
            }
            FilterExpression::Float { value } => {
                Ok(FilterExpressionResult::FloatLiteral(value.clone()))
            }
            FilterExpression::Not { expression } => expression.evaluate(context).map(|rv| {
                if !is_truthy(&rv) {
                    FilterExpressionResult::True
                } else {
                    FilterExpressionResult::False
                }
            }),
            FilterExpression::Logical {
                left,
                operator,
                right,
            } => {
                if logical(
                    &left.evaluate(context)?,
                    operator,
                    &right.evaluate(context)?,
                ) {
                    Ok(FilterExpressionResult::True)
                } else {
                    Ok(FilterExpressionResult::False)
                }
            }
            FilterExpression::Comparison {
                left,
                operator,
                right,
            } => todo!(),
            FilterExpression::RelativeQuery { query } => todo!(),
            FilterExpression::RootQuery { query } => todo!(),
            FilterExpression::Function { name, args } => todo!(),
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

fn visit<'a, 'b>(node: &'b Node<'a>) -> NodeList<'a> {
    let mut nodes: NodeList = vec![node.clone()];

    match node.value {
        Value::Object(obj) => {
            obj.iter()
                .for_each(|(k, v)| nodes.extend(visit(&node.new_child_member(v, k))));
        }
        Value::Array(arr) => arr
            .iter()
            .enumerate()
            .for_each(|(i, e)| nodes.extend(visit(&node.new_child_element(e, i)))),
        _ => (),
    }

    nodes
}

fn slice<'a>(
    array: &'a Vec<Value>,
    start: Option<i64>,
    stop: Option<i64>,
    step: Option<i64>,
) -> Vec<(i64, &'a Value)> {
    let array_length = array.len() as i64; // TODO: try_from
    if array_length == 0 {
        return Vec::new();
    }

    let n_step = step.unwrap_or(1);

    if n_step == 0 {
        return Vec::new();
    }

    let n_start = match start {
        Some(i) => {
            if i < 0 {
                cmp::max(array_length + i, 0)
            } else {
                cmp::min(i, array_length - 1)
            }
        }
        None => {
            if n_step < 0 {
                array_length - 1
            } else {
                0
            }
        }
    };

    let n_stop = match stop {
        Some(i) => {
            if i < 0 {
                cmp::max(array_length + i, -1)
            } else {
                cmp::min(i, array_length)
            }
        }
        None => {
            if n_step < 0 {
                -1
            } else {
                array_length
            }
        }
    };

    let mut sliced_array: Vec<(i64, &Value)> = Vec::new();

    // TODO: try_from instead of as
    if n_step > 0 {
        for i in (n_start..n_stop).step_by(n_step as usize) {
            sliced_array.push((i, array.get(i as usize).unwrap()));
        }
    } else {
        let mut i = n_start;
        while i > n_stop {
            sliced_array.push((i, array.get(i as usize).unwrap()));
            i += n_step;
        }
    }

    sliced_array
}

pub fn is_truthy(rv: &FilterExpressionResult) -> bool {
    match rv {
        FilterExpressionResult::Nothing => false,
        FilterExpressionResult::Nodes(nodes) => !nodes.is_empty(),
        FilterExpressionResult::False => false,
        _ => true,
    }
}

fn logical(
    left: &FilterExpressionResult,
    op: &LogicalOperator,
    right: &FilterExpressionResult,
) -> bool {
    match op {
        LogicalOperator::And => is_truthy(left) && is_truthy(right),
        LogicalOperator::Or => is_truthy(left) || is_truthy(right),
    }
}
