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

use crate::{
    env::Environment, errors::JSONPathError, function::ExpressionType, parser::JSONPathParser,
};

lazy_static! {
    static ref PARSER: JSONPathParser = JSONPathParser::new();
}

// TODO: Change location to be a sequence of parts, so as to derive keys
// TODO: implement fn path to get normalized path from location parts

#[derive(Debug, Clone, PartialEq)]
pub struct Node<'a> {
    pub value: &'a Value,
    pub location: String,
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

struct QueryContext<'a, 'b> {
    env: &'b Environment,
    root: &'a Value,
}

// TODO: UInt
#[derive(Debug, PartialEq)]
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
                } else if n.is_i64() {
                    FilterExpressionResult::Int(n.as_i64().unwrap())
                } else {
                    FilterExpressionResult::Int(n.as_i64().unwrap()) // XXX:
                }
            }
            Value::String(s) => FilterExpressionResult::String(s.to_owned()),
            Value::Array(_) => FilterExpressionResult::Array(value),
            Value::Object(_) => FilterExpressionResult::Object(value),
        }
    }
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

    pub fn find<'a, 'b>(
        &self,
        value: &'a Value,
        env: &'b Environment,
    ) -> Result<NodeList<'a>, JSONPathError> {
        let context = QueryContext { root: value, env };

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

    // Same as `find`, but uses explicit `for` loops and vectors to collect intermediate nodes.
    pub fn find_loop<'a, 'b>(
        &self,
        value: &'a Value,
        env: &'b Environment,
    ) -> Result<NodeList<'a>, JSONPathError> {
        let context = QueryContext { root: value, env };

        let mut nodes: NodeList<'a> = vec![Node {
            value,
            location: String::from("$"),
        }];

        for segment in self.segments.iter() {
            nodes = segment.resolve_loop(nodes, &context)?;
        }

        Ok(nodes)
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
            Segment::Child { selectors } => nodes
                .iter()
                .flat_map(|node| selectors.iter().map(|s| s.resolve(node, context)))
                .flatten_ok()
                .collect(),
            Segment::Recursive { selectors } => nodes
                .iter()
                .flat_map(|n| visit(n))
                .flat_map(|node| selectors.iter().map(move |s| s.resolve(&node, context)))
                .flatten_ok()
                .collect(),
            Segment::Eoi {} => Ok(nodes),
        }
    }

    fn resolve_loop<'a>(
        &self,
        nodes: NodeList<'a>,
        context: &QueryContext,
    ) -> Result<NodeList<'a>, JSONPathError> {
        let mut _nodes: NodeList<'a> = Vec::new();
        match self {
            Segment::Child { selectors } => {
                for node in nodes.iter() {
                    for selector in selectors {
                        _nodes.extend(selector.resolve(node, context)?)
                    }
                }
            }
            Segment::Recursive { selectors } => {
                for node in nodes.iter() {
                    for _node in visit(node).iter() {
                        for selector in selectors {
                            _nodes.extend(selector.resolve_loop(_node, context)?)
                        }
                    }
                }
            }
            Segment::Eoi {} => _nodes = nodes,
        }
        Ok(_nodes)
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
                if let Some(array) = node.value.as_array() {
                    let norm = norm_index(*index, array.len());
                    if let Some(v) = array.get(norm) {
                        Ok(vec![node.new_child_element(v, norm)])
                    } else {
                        Ok(Vec::new())
                    }
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
                    .map(|(i, v)| expression.evaluate(v, context).map(|r| (i, v, r)))
                    .filter_ok(|(_, _, r)| is_truthy_ref(r))
                    .map_ok(|(i, v, _)| node.new_child_element(v, i))
                    .collect(),
                Value::Object(obj) => obj
                    .iter()
                    .map(|(k, v)| expression.evaluate(v, context).map(|r| (k, v, r)))
                    .filter_ok(|(_, _, r)| is_truthy_ref(r))
                    .map_ok(|(k, v, _)| node.new_child_member(v, k))
                    .collect(),
                _ => Ok(Vec::new()),
            },
        }
    }

    fn resolve_loop<'a, 'b>(
        &self,
        node: &'b Node<'a>,
        context: &QueryContext,
    ) -> Result<NodeList<'a>, JSONPathError> {
        let mut nodes: NodeList = Vec::new();
        match self {
            Selector::Name { name } => {
                if let Some(v) = node.value.get(name) {
                    nodes.push(node.new_child_member(v, name));
                }
            }
            Selector::Index { index } => {
                if let Some(array) = node.value.as_array() {
                    let norm = norm_index(*index, array.len());
                    if let Some(v) = array.get(norm) {
                        nodes.push(node.new_child_element(v, norm));
                    }
                }
            }
            Selector::Slice { start, stop, step } => {
                if let Some(array) = node.value.as_array() {
                    for (i, element) in slice(array, *start, *stop, *step) {
                        nodes.push(node.new_child_element(element, i as usize));
                    }
                }
            }
            Selector::Wild {} => match node.value {
                Value::Array(array) => {
                    for (i, element) in array.iter().enumerate() {
                        nodes.push(node.new_child_element(element, i));
                    }
                }
                Value::Object(obj) => {
                    for (k, v) in obj {
                        nodes.push(node.new_child_member(v, k));
                    }
                }
                _ => (),
            },
            Selector::Filter { expression } => match node.value {
                Value::Array(array) => {
                    for (i, element) in array.iter().enumerate() {
                        if is_truthy(expression.evaluate(element, context)?) {
                            nodes.push(node.new_child_element(element, i));
                        }
                    }
                }
                Value::Object(obj) => {
                    for (k, v) in obj {
                        if is_truthy(expression.evaluate(v, context)?) {
                            nodes.push(node.new_child_member(v, k));
                        }
                    }
                }

                _ => (),
            },
        }
        Ok(nodes)
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
    fn evaluate<'a, 'b: 'a>(
        &self,
        current: &'a Value,
        context: &QueryContext<'a, 'b>,
    ) -> Result<FilterExpressionResult<'a>, JSONPathError> {
        match self {
            FilterExpression::True => Ok(FilterExpressionResult::Bool(true)),
            FilterExpression::False => Ok(FilterExpressionResult::Bool(false)),
            FilterExpression::Null => Ok(FilterExpressionResult::Null),
            FilterExpression::String { value } => {
                Ok(FilterExpressionResult::String(value.to_owned()))
            }
            FilterExpression::Int { value } => Ok(FilterExpressionResult::Int(*value)),
            FilterExpression::Float { value } => Ok(FilterExpressionResult::Float(*value)),
            FilterExpression::Not { expression } => {
                expression.evaluate(current, context).map(|rv| {
                    if !is_truthy(rv) {
                        FilterExpressionResult::Bool(true)
                    } else {
                        FilterExpressionResult::Bool(false)
                    }
                })
            }
            FilterExpression::Logical {
                left,
                operator,
                right,
            } => {
                if logical(
                    left.evaluate(current, context)?,
                    operator,
                    right.evaluate(current, context)?,
                ) {
                    Ok(FilterExpressionResult::Bool(true))
                } else {
                    Ok(FilterExpressionResult::Bool(false))
                }
            }
            FilterExpression::Comparison {
                left,
                operator,
                right,
            } => {
                if compare(
                    left.evaluate(current, context)?,
                    operator,
                    right.evaluate(current, context)?,
                ) {
                    Ok(FilterExpressionResult::Bool(true))
                } else {
                    Ok(FilterExpressionResult::Bool(false))
                }
            }
            FilterExpression::RelativeQuery { query } => Ok(FilterExpressionResult::Nodes(
                query.find(current, context.env)?,
            )),
            FilterExpression::RootQuery { query } => Ok(FilterExpressionResult::Nodes(
                query.find(context.root, context.env)?,
            )),
            FilterExpression::Function { name, args } => {
                let fn_ext = context.env.function_register.get(name).ok_or_else(|| {
                    JSONPathError::name(format!("missing function definition for {}", name))
                })?;

                let _args: Result<Vec<_>, _> = args
                    .iter()
                    .map(|expr| expr.evaluate(current, context))
                    .enumerate()
                    .map(|(i, rv)| unpack_result(rv?, &fn_ext.sig().param_types, i))
                    .collect();

                Ok(fn_ext.call(_args?))
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

pub fn is_truthy(rv: FilterExpressionResult) -> bool {
    match rv {
        FilterExpressionResult::Nothing => false,
        FilterExpressionResult::Nodes(nodes) => !nodes.is_empty(),
        FilterExpressionResult::Bool(v) => v == true,
        _ => true,
    }
}

pub fn is_truthy_ref(rv: &FilterExpressionResult) -> bool {
    match rv {
        FilterExpressionResult::Nothing => false,
        FilterExpressionResult::Nodes(nodes) => !nodes.is_empty(),
        FilterExpressionResult::Bool(v) => *v == true,
        _ => true,
    }
}

fn logical(
    left: FilterExpressionResult,
    op: &LogicalOperator,
    right: FilterExpressionResult,
) -> bool {
    match op {
        LogicalOperator::And => is_truthy(left) && is_truthy(right),
        LogicalOperator::Or => is_truthy(left) || is_truthy(right),
    }
}

fn nodes_or_singular<'a>(rv: FilterExpressionResult<'a>) -> FilterExpressionResult<'a> {
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

fn compare(
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

fn unpack_result<'a>(
    rv: FilterExpressionResult<'a>,
    param_types: &[ExpressionType],
    index: usize,
) -> Result<FilterExpressionResult<'a>, JSONPathError> {
    if matches!(param_types.get(index).unwrap(), ExpressionType::Nodes) {
        return Ok(rv);
    }

    match &rv {
        FilterExpressionResult::Nodes(nodes) => match nodes.len() {
            0 => Ok(FilterExpressionResult::Nothing),
            1 => Ok(FilterExpressionResult::from_json_value(
                nodes.first().unwrap().value,
            )),
            _ => Ok(rv),
        },
        _ => Ok(rv),
    }
}

fn norm_index(index: i64, length: usize) -> usize {
    if index < 0 && length >= index.abs() as usize {
        (length as i64 + index) as usize
    } else {
        index as usize
    }
}
