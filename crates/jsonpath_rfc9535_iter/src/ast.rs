//! Structs and enums that make up a JSONPath query syntax tree.
//!
//! A [`Query`] contains zero or more [`Segment`]s, and each segment contains one
//! or more [`Selector`]s. When a segment includes a _filter selector_, that
//! filter selector is a tree of [`FilterExpression`]s.
//!
//! [RFC 9535]: https://datatracker.ietf.org/doc/html/rfc9535
use lazy_static::lazy_static;
use serde_json::{Map, Value};
use std::{
    cmp,
    fmt::{self, Write},
    iter,
    rc::Rc,
    slice::Iter,
};

use crate::{
    env::Environment, errors::JSONPathError, function::ExpressionType, parser::JSONPathParser,
};

lazy_static! {
    static ref PARSER: JSONPathParser = JSONPathParser::new();
}

pub type NodeList<'v> = Vec<&'v Value>;
type It<'v> = Box<dyn Iterator<Item = &'v Value> + 'v>;

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

impl<'v> FilterExpressionResult<'v> {
    pub fn from_json_value(value: &'v Value) -> Self {
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

#[derive(Debug, Clone)]
pub struct Query {
    pub segments: Vec<Segment>,
}

impl Query {
    pub fn new(segments: Vec<Segment>) -> Self {
        Query { segments }
    }

    // pub fn standard(expr: &str) -> Result<Self, JSONPathError> {
    //     PARSER.parse(expr)
    // }

    // pub fn find<'v, 'e: 'v>(&self, value: &'v Value, env: Rc<Environment>) -> QueryIter<'e, 'v> {
    //     QueryIter::new(env, value, self)
    // }

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

#[derive(Debug, Clone)]
pub enum Segment {
    Child { selectors: Vec<Selector> },
    Recursive { selectors: Vec<Selector> },
    Eoi,
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

pub struct QueryIter<'v> {
    env: Rc<Environment>,
    root: &'v Value,
    it: SegmentIter<'v>,
}

impl<'v> Iterator for QueryIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

// TODO: implement From for Query

impl<'v> QueryIter<'v> {
    pub fn new(env: Rc<Environment>, root: &'v Value, query: Query) -> Self {
        let init = SegmentIter {
            its: Vec::new(),
            it: SelectorIter {
                env: env.clone(),
                root,
                it: Box::new(iter::once(root)),
            },
        };

        let it = query
            .segments
            .clone()
            .into_iter()
            .filter(|s| !matches!(s, Segment::Eoi {}))
            .fold(init, |values, segment| {
                SegmentIter::new(env.clone(), root, segment, Box::new(values))
            });

        Self {
            env: env.clone(),
            root,
            it,
        }
    }
}

pub struct SegmentIter<'v> {
    its: Vec<SelectorIter<'v>>,
    it: SelectorIter<'v>,
}

impl<'v> Iterator for SegmentIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some(v) => Some(v),
            None => match self.its.pop() {
                Some(it) => {
                    self.it = it;
                    self.next()
                }
                None => None,
            },
        }
    }
}

impl<'v> SegmentIter<'v> {
    fn new<'q: 'v>(
        env: Rc<Environment>,
        root: &'v Value,
        segment: Segment,
        values: It<'v>,
    ) -> Self {
        let mut its: Vec<SelectorIter<'v>> = Vec::new();

        match segment {
            Segment::Child { ref selectors } => {
                for value in values {
                    for selector in selectors {
                        its.push(SelectorIter::new(
                            env.clone(),
                            root,
                            selector.clone(),
                            value,
                        ))
                    }
                }
            }
            Segment::Recursive { selectors } => todo!(),
            Segment::Eoi {} => unreachable!(),
        };

        // TODO: or use a queue
        // TODO: flatten
        its.reverse();
        let it = its.pop().unwrap();
        Self { its, it }
    }
}

pub struct SelectorIter<'v> {
    env: Rc<Environment>,
    root: &'v Value,
    it: It<'v>,
}

impl<'v> Iterator for SelectorIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

impl<'v> SelectorIter<'v> {
    fn new(env: Rc<Environment>, root: &'v Value, selector: Selector, value: &'v Value) -> Self {
        let it: It<'v> = match selector {
            Selector::Name { name } => {
                if let Some(v) = value.get(name) {
                    Box::new(iter::once(v))
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Index { index } => {
                if let Some(array) = value.as_array() {
                    let norm = norm_index(index, array.len());
                    if let Some(v) = array.get(norm) {
                        Box::new(iter::once(v))
                    } else {
                        Box::new(iter::empty())
                    }
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Slice { start, stop, step } => {
                if let Some(array) = value.as_array() {
                    Box::new(slice(array, start, stop, step).into_iter().map(|(_, v)| v))
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Wild {} => match value {
                Value::Array(arr) => Box::new(arr.iter().enumerate().map(|(_, v)| v)),
                Value::Object(obj) => Box::new(obj.iter().map(|(_k, v)| v)),
                _ => Box::new(iter::empty()),
            },
            Selector::Filter { expression } => match value {
                Value::Array(arr) => {
                    Box::new(ArrayFilterIter::new(env.clone(), root, *expression, arr))
                }
                Value::Object(obj) => {
                    Box::new(ObjectFilterIter::new(env.clone(), root, *expression, obj))
                }
                _ => Box::new(iter::empty()),
            },
        };

        Self {
            env: env.clone(),
            root,
            it,
        }
    }
}

#[derive(Debug, Clone)]
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

pub struct ArrayFilterIter<'v> {
    env: Rc<Environment>,
    root: &'v Value,
    expr: FilterExpression,
    it: Iter<'v, Value>,
}

impl<'v> Iterator for ArrayFilterIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some(value) => {
                if is_truthy(self.expr.evaluate(self.env.clone(), self.root, value)) {
                    Some(value)
                } else {
                    self.next()
                }
            }
            None => None,
        }
    }
}

impl<'v> ArrayFilterIter<'v> {
    fn new(
        env: Rc<Environment>,
        root: &'v Value,
        expr: FilterExpression,
        arr: &'v Vec<Value>,
    ) -> Self {
        Self {
            env: env.clone(),
            root,
            expr,
            it: arr.iter(),
        }
    }
}

pub struct ObjectFilterIter<'v> {
    env: Rc<Environment>,
    root: &'v Value,
    expr: FilterExpression,
    it: serde_json::map::Iter<'v>,
}

impl<'v> Iterator for ObjectFilterIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some((_k, v)) => {
                if is_truthy(self.expr.evaluate(self.env.clone(), self.root, v)) {
                    Some(v)
                } else {
                    self.next()
                }
            }
            None => None,
        }
    }
}

impl<'v> ObjectFilterIter<'v> {
    fn new(
        env: Rc<Environment>,
        root: &'v Value,
        expr: FilterExpression,
        obj: &'v Map<String, Value>,
    ) -> Self {
        Self {
            env,
            root,
            expr,
            it: obj.iter(),
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
    fn evaluate<'a: 'v, 'v>(
        &'a self,
        env: Rc<Environment>,
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
                if !is_truthy(expression.evaluate(env.clone(), root, current)) {
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
                    left.evaluate(env.clone(), root, current),
                    operator,
                    right.evaluate(env.clone(), root, current),
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
                    left.evaluate(env.clone(), root, current),
                    operator,
                    right.evaluate(env.clone(), root, current),
                ) {
                    FilterExpressionResult::Bool(true)
                } else {
                    FilterExpressionResult::Bool(false)
                }
            }
            FilterExpression::RelativeQuery { query } => FilterExpressionResult::Nodes(
                QueryIter::new(env.clone(), current, *query.clone()).collect(),
            ),
            FilterExpression::RootQuery { query } => FilterExpressionResult::Nodes(
                QueryIter::new(env.clone(), root, *query.clone()).collect(),
            ),
            FilterExpression::Function { name, args } => {
                let fn_ext = env
                    .function_register
                    .get(name)
                    .expect(&format!("unknown function '{}'", name));

                let _args = args
                    .iter()
                    .map(|expr| expr.evaluate(env.clone(), root, current))
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

fn visit<'v>(value: &'v Value) -> NodeList<'v> {
    let mut values: NodeList = vec![value];

    match value {
        Value::Object(obj) => {
            obj.iter().for_each(|(_k, v)| values.extend(visit(v)));
        }
        Value::Array(arr) => arr
            .iter()
            .enumerate()
            .for_each(|(_i, e)| values.extend(visit(e))),
        _ => (),
    }

    values
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
                FilterExpressionResult::from_json_value(nodes.first().unwrap())
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

fn unpack_result<'v>(
    rv: FilterExpressionResult<'v>,
    param_types: &[ExpressionType],
    index: usize,
) -> FilterExpressionResult<'v> {
    if matches!(param_types.get(index).unwrap(), ExpressionType::Nodes) {
        return rv;
    }

    match &rv {
        FilterExpressionResult::Nodes(values) => match values.len() {
            0 => FilterExpressionResult::Nothing,
            1 => FilterExpressionResult::from_json_value(values.first().unwrap()),
            _ => rv,
        },
        _ => rv,
    }
}

fn norm_index(index: i64, length: usize) -> usize {
    if index < 0 && length >= index.abs() as usize {
        (length as i64 + index) as usize
    } else {
        index as usize
    }
}
