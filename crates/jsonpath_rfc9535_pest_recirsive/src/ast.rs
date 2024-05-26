use std::fmt::{self, Write};

use lazy_static::lazy_static;

use crate::{errors::JSONPathError, JSONPathParser};

#[derive(Debug)]
pub struct Query {
    pub ast: Segment,
}

#[derive(Debug)]
pub enum Segment {
    Root {},
    Child {
        left: Box<Segment>,
        selectors: Vec<Selector>,
    },
    Recursive {
        left: Box<Segment>,
        selectors: Vec<Selector>,
    },
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
    Wild {},
    Filter {
        expression: Box<FilterExpression>,
    },
}

#[derive(Debug)]
pub enum FilterExpression {
    True_ {},
    False_ {},
    Null {},
    StringLiteral {
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

#[derive(Debug)]
pub enum LogicalOperator {
    And,
    Or,
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

lazy_static! {
    static ref PARSER: JSONPathParser = JSONPathParser::new();
}

impl Query {
    pub fn standard(expr: &str) -> Result<Self, JSONPathError> {
        PARSER.parse(expr)
    }

    pub fn is_singular(&self) -> bool {
        self.ast.is_singular()
    }
}

impl Segment {
    // Returns `true` if this query can resolve to at most one node, or `false` otherwise.
    pub fn is_singular(&self) -> bool {
        match self {
            Segment::Child { left, selectors } => {
                selectors.len() == 1
                    && selectors.first().is_some_and(|selector| {
                        matches!(selector, Selector::Name { .. } | Selector::Index { .. })
                    })
                    && left.is_singular()
            }
            Segment::Recursive { .. } => false,
            Segment::Root {} => true,
        }
    }
}

impl FilterExpression {
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            FilterExpression::True_ { .. }
                | FilterExpression::False_ { .. }
                | FilterExpression::Null { .. }
                | FilterExpression::StringLiteral { .. }
                | FilterExpression::Int { .. }
                | FilterExpression::Float { .. }
        )
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.ast)
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Segment::Child { left, selectors } => write!(
                f,
                "{}[{}]",
                left,
                selectors
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Segment::Recursive { left, selectors } => write!(
                f,
                "{}..[{}]",
                left,
                selectors
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Segment::Root {} => Ok(()),
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

impl fmt::Display for FilterExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterExpression::True_ { .. } => f.write_str("true"),
            FilterExpression::False_ { .. } => f.write_str("false"),
            FilterExpression::Null { .. } => f.write_str("null"),
            FilterExpression::StringLiteral { value, .. } => write!(f, "'{value}'"),
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
                write!(f, "@{}", query.ast)
            }
            FilterExpression::RootQuery { query, .. } => {
                write!(f, "${}", query.ast)
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

impl fmt::Display for LogicalOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicalOperator::And => f.write_str("&&"),
            LogicalOperator::Or => f.write_str("||"),
        }
    }
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
