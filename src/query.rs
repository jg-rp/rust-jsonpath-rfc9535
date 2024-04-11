use std::fmt::{self, Write};

use crate::{env::Env, errors::JSONPathError, lexer::lex, parser::Parser, token::Token};

use lazy_static::lazy_static;

lazy_static! {
    static ref PARSER: Parser = Parser::new(Env::standard());
}

#[derive(Debug)]
pub struct Query {
    pub segments: Vec<Segment>,
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

impl Query {
    pub fn new(expr: &str) -> Result<Self, JSONPathError> {
        Ok(Query {
            segments: PARSER.parse(lex(expr)?)?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.segments.len() == 0
    }

    pub fn is_singular(&self) -> bool {
        for segment in self.segments.iter() {
            match segment {
                Segment::Child { selectors, .. } => {
                    // A single name or index selector?
                    if selectors.len() == 1
                        && selectors.first().is_some_and(|s| {
                            matches!(s, Selector::Name { .. } | Selector::Index { .. })
                        })
                    {
                        continue;
                    } else {
                        return false;
                    }
                }
                Segment::Recursive { .. } => {
                    return false;
                }
            }
        }

        true
    }
}

#[derive(Debug)]
pub enum Segment {
    Child {
        token: Token,
        selectors: Vec<Selector>,
    },
    Recursive {
        token: Token,
        selectors: Vec<Selector>,
    },
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
        }
    }
}

#[derive(Debug)]
pub enum Selector {
    Name {
        token: Token,
        name: String,
    },
    Index {
        token: Token,
        index: isize,
    },
    Slice {
        token: Token,
        start: Option<isize>,
        stop: Option<isize>,
        step: Option<isize>,
    },
    Wild {
        token: Token,
    },
    Filter {
        token: Token,
        expression: Box<FilterExpression>,
    },
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Selector::Name { name, .. } => write!(f, "'{name}'"),
            Selector::Index { index, .. } => write!(f, "{index}"),
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
pub enum FilterExpressionType {
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
    Boolean {
        expression: Box<FilterExpression>,
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
pub struct FilterExpression {
    pub token: Token,
    pub kind: FilterExpressionType,
}

impl FilterExpression {
    pub fn new(token: Token, kind: FilterExpressionType) -> Self {
        FilterExpression { token, kind }
    }

    pub fn is_literal(&self) -> bool {
        use FilterExpressionType::*;
        matches!(
            self.kind,
            True | False | Null | String { .. } | Int { .. } | Float { .. }
        )
    }
}

impl fmt::Display for FilterExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FilterExpressionType::True => f.write_str("true"),
            FilterExpressionType::False => f.write_str("false"),
            FilterExpressionType::Null => f.write_str("null"),
            FilterExpressionType::String { value } => write!(f, "\"{value}\""),
            FilterExpressionType::Int { value } => write!(f, "{value}"),
            FilterExpressionType::Float { value } => write!(f, "{value}"),
            FilterExpressionType::Boolean { expression } => write!(f, "{expression}"),
            FilterExpressionType::Not { expression } => write!(f, "!{expression}"),
            FilterExpressionType::Logical {
                left,
                operator,
                right,
            } => write!(f, "({left} {operator} {right})"),
            FilterExpressionType::Comparison {
                left,
                operator,
                right,
            } => write!(f, "{left} {operator} {right}"),
            FilterExpressionType::RelativeQuery { query } => {
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
            FilterExpressionType::RootQuery { query } => {
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
            FilterExpressionType::Function { name, args } => {
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
