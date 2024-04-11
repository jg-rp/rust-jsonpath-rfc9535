use std::{iter::Peekable, vec::IntoIter};

use crate::{
    env::{Env, ExpressionType, FunctionSignature},
    errors::{JSONPathError, JSONPathErrorType},
    query::{
        ComparisonOperator, FilterExpression, FilterExpressionType, LogicalOperator, Query,
        Segment, Selector,
    },
    token::{Token, TokenType},
};

use TokenType::*;

const EOF_TOKEN: Token = Token {
    kind: EOF,
    index: 0,
};

type Tokens = Peekable<IntoIter<Token>>;

const PRECEDENCE_LOWEST: u8 = 1;
const PRECEDENCE_LOGICAL_OR: u8 = 3;
const PRECEDENCE_LOGICAL_AND: u8 = 4;
const PRECEDENCE_RELATIONAL: u8 = 5;
const PRECEDENCE_LOGICAL_NOT: u8 = 7;

pub struct Parser {
    env: Env,
}

impl Parser {
    pub fn new(env: Env) -> Self {
        Parser { env }
    }

    pub fn parse(&self, tokens: Vec<Token>) -> Result<Vec<Segment>, JSONPathError> {
        // TODO: try an iterator wrapper that keeps returning EOF when exhausted
        let mut it = tokens.into_iter().peekable();

        match it.next().unwrap_or(EOF_TOKEN) {
            Token { kind: Root, .. } => {
                let segments = self.parse_segments(&mut it)?;
                // parse_query should have consumed all tokens
                match it.next() {
                    Some(Token { kind: EOF, .. }) | None => Ok(segments),
                    Some(token) => Err(JSONPathError::syntax(
                        format!("expected end of query, found {}", token.kind),
                        token.index,
                    )),
                }
            }
            token => Err(JSONPathError::syntax(
                format!("expected '$', found {}", token.kind),
                token.index,
            )),
        }
    }

    fn parse_segments(&self, it: &mut Tokens) -> Result<Vec<Segment>, JSONPathError> {
        let mut segments: Vec<Segment> = Vec::new();
        loop {
            match it.peek().unwrap_or(&EOF_TOKEN).kind {
                DoubleDot => {
                    let token = it.next().unwrap();
                    let selectors = self.parse_selectors(it)?;
                    segments.push(Segment::Recursive { token, selectors });
                }
                LBracket | Name { .. } | Wild => {
                    let token = (*it.peek().unwrap()).clone();
                    let selectors = self.parse_selectors(it)?;
                    segments.push(Segment::Child { token, selectors });
                }
                _ => {
                    break;
                }
            }
        }

        Ok(segments)
    }

    fn parse_selectors(&self, it: &mut Tokens) -> Result<Vec<Selector>, JSONPathError> {
        match it.peek().unwrap_or(&EOF_TOKEN) {
            Token {
                kind: Name { value },
                index,
            } => {
                let name = unescape_string(value, &index)?;
                let token = it.next().unwrap();
                Ok(vec![Selector::Name { token, name }])
            }
            Token { kind: Wild, .. } => Ok(vec![Selector::Wild {
                token: it.next().unwrap(),
            }]),
            Token { kind: LBracket, .. } => self.parse_bracketed(it),
            _ => Ok(Vec::new()),
        }
    }

    fn parse_bracketed(&self, it: &mut Tokens) -> Result<Vec<Selector>, JSONPathError> {
        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Some(Token { kind: LBracket, .. })),
            "expected the start of a bracketed selection"
        );

        let token = it.next().unwrap(); // LBracket
        let mut selectors: Vec<Selector> = Vec::new();

        loop {
            match it.peek().unwrap_or(&EOF_TOKEN) {
                Token { kind: RBracket, .. } => {
                    it.next();
                    break;
                }
                Token {
                    kind: Index { .. } | Colon,
                    ..
                } => {
                    let selector = self.parse_slice_or_index(it)?;
                    selectors.push(selector);
                }
                Token {
                    kind: DoubleQuoteString { value },
                    index,
                } => {
                    let name = unescape_string(value, &index)?;
                    let token = it.next().unwrap();
                    selectors.push(Selector::Name { token, name });
                }
                Token {
                    kind: SingleQuoteString { value },
                    index,
                } => {
                    let name = unescape_string(&value.replace("\\'", "'"), index)?;
                    let token = it.next().unwrap();
                    selectors.push(Selector::Name { token, name });
                }
                Token { kind: Wild, .. } => {
                    let token = it.next().unwrap();
                    selectors.push(Selector::Wild { token });
                }
                Token { kind: Filter, .. } => {
                    let selector = self.parse_filter(it)?;
                    selectors.push(selector);
                }
                Token { kind: EOF, .. } => {
                    return Err(JSONPathError::syntax(
                        String::from("unexpected end of query"),
                        token.index,
                    ));
                }
                token => {
                    return Err(JSONPathError::syntax(
                        format!("unexpected selector token {}", token.kind),
                        token.index,
                    ));
                }
            }

            #[cfg(debug_assertions)]
            debug_assert!(
                matches!(
                    it.peek(),
                    Some(Token {
                        kind: Comma | TokenType::RBracket,
                        ..
                    })
                ),
                "expected a comma or the end of a bracketed selection"
            );

            // expect a comma or closing bracket
            match it.peek() {
                Some(Token { kind: RBracket, .. }) => continue,
                Some(Token { kind: Comma, .. }) => {
                    // eat comma
                    it.next();
                }
                Some(token) => {
                    return Err(JSONPathError::new(
                        JSONPathErrorType::SyntaxError,
                        format!("expected a comma or closing bracket, found {}", token.kind),
                        token.index,
                    ));
                }
                None => continue,
            }
        }

        if selectors.len() == 0 {
            return Err(JSONPathError::new(
                JSONPathErrorType::SyntaxError,
                String::from("empty bracketed selection"),
                token.index,
            ));
        }

        Ok(selectors)
    }

    fn parse_slice_or_index(&self, it: &mut Tokens) -> Result<Selector, JSONPathError> {
        let token = it.next().unwrap(); // index or colon

        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(
                token,
                Token {
                    kind: Colon | Index { .. },
                    ..
                }
            ),
            "expected an index or slice"
        );

        if token.kind == Colon || it.peek().unwrap_or(&EOF_TOKEN).kind == Colon {
            // a slice
            let mut start: Option<isize> = None;
            let mut stop: Option<isize> = None;
            let mut step: Option<isize> = None;

            // 1: or :
            if let Token {
                kind: Index { ref value },
                index,
            } = &token
            {
                validate_index(value, *index)?;
                start = Some(value.parse::<isize>().map_err(|_| {
                    JSONPathError::syntax(String::from("invalid start index"), *index)
                })?);
                it.next(); // eat colon
            }

            // 1 or 1: or : or ?
            if matches!(it.peek().unwrap_or(&EOF_TOKEN).kind, Index { .. } | Colon) {
                if let Token {
                    kind: Index { ref value },
                    index,
                } = it.next().unwrap()
                {
                    validate_index(value, index)?;
                    stop = Some(value.parse::<isize>().map_err(|_| {
                        JSONPathError::syntax(String::from("invalid stop index"), index)
                    })?);
                    if it.peek().unwrap_or(&EOF_TOKEN).kind == Colon {
                        it.next(); // eat colon
                    }
                }
            }

            // 1 or ?
            if matches!(it.peek().unwrap_or(&EOF_TOKEN).kind, Index { .. }) {
                if let Token {
                    kind: Index { ref value },
                    index,
                } = it.next().unwrap()
                {
                    validate_index(value, index)?;
                    step =
                        Some(value.parse::<isize>().map_err(|_| {
                            JSONPathError::syntax(String::from("invalid step"), index)
                        })?);
                }
            }

            Ok(Selector::Slice {
                token,
                start,
                stop,
                step,
            })
        } else {
            // an index
            match token {
                Token {
                    kind: Index { ref value },
                    ..
                } => {
                    if value.len() > 1 && (value.starts_with("0") || value.starts_with("-0")) {
                        return Err(JSONPathError::syntax(
                            String::from("unexpected leading zero in index selector"),
                            token.index,
                        ));
                    }
                    let index = value.parse::<isize>().unwrap();
                    Ok(Selector::Index { token, index })
                }
                tok => Err(JSONPathError::syntax(
                    format!("expected an index, found {}", tok.kind),
                    tok.index,
                )),
            }
        }
    }

    fn parse_filter(&self, it: &mut Tokens) -> Result<Selector, JSONPathError> {
        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Some(Token { kind: Filter, .. })),
            "expected a filter"
        );

        let token = it.next().unwrap();
        let expr = self.parse_filter_expression(it, PRECEDENCE_LOWEST)?;

        if let FilterExpression {
            kind: FilterExpressionType::Function { ref name, .. },
            ..
        } = expr
        {
            match self.env.functions.get(name) {
                Some(FunctionSignature {
                    return_type: ExpressionType::Value,
                    ..
                }) => {
                    return Err(JSONPathError::typ(
                        format!("result of {}() must be compared", name),
                        expr.token.index,
                    ));
                }
                _ => (),
            }
        }

        if expr.is_literal() {
            return Err(JSONPathError::typ(
                String::from("filter expression literals must be compared"),
                expr.token.index,
            ));
        }

        Ok(Selector::Filter {
            token,
            expression: Box::new(expr),
        })
    }

    fn parse_not_expression(&self, it: &mut Tokens) -> Result<FilterExpression, JSONPathError> {
        let token = it.next().unwrap();
        let expr = self.parse_filter_expression(it, PRECEDENCE_LOGICAL_NOT)?;
        Ok(FilterExpression::new(
            token,
            FilterExpressionType::Not {
                expression: Box::new(expr),
            },
        ))
    }

    fn parse_infix_expression(
        &self,
        it: &mut Tokens,
        left: FilterExpression,
    ) -> Result<FilterExpression, JSONPathError> {
        let token = it.next().unwrap();
        let precedence = self.precedence(&token.kind);
        let right = self.parse_filter_expression(it, precedence)?;

        match token.kind {
            And => {
                // TODO: error if left or right is an expression literal
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Logical {
                        left: Box::new(left),
                        operator: LogicalOperator::And,
                        right: Box::new(right),
                    },
                ))
            }
            Or => {
                // TODO: error if left or right is an expression literal
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Logical {
                        left: Box::new(left),
                        operator: LogicalOperator::Or,
                        right: Box::new(right),
                    },
                ))
            }
            Eq => {
                // TODO: error if non comparable
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Eq,
                        right: Box::new(right),
                    },
                ))
            }
            Ge => {
                // TODO: error if non comparable
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Ge,
                        right: Box::new(right),
                    },
                ))
            }
            Gt => {
                // TODO: error if non comparable
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Gt,
                        right: Box::new(right),
                    },
                ))
            }
            Le => {
                // TODO: error if non comparable
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Le,
                        right: Box::new(right),
                    },
                ))
            }
            Lt => {
                // TODO: error if non comparable
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Lt,
                        right: Box::new(right),
                    },
                ))
            }
            Ne => {
                // TODO: error if non comparable
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Ne,
                        right: Box::new(right),
                    },
                ))
            }
            _ => Err(JSONPathError::syntax(
                format!("unexpected infix operator {}", token.kind),
                token.index,
            )),
        }
    }

    fn parse_grouped_expression(&self, it: &mut Tokens) -> Result<FilterExpression, JSONPathError> {
        it.next(); // eat open paren
        let mut expr = self.parse_filter_expression(it, PRECEDENCE_LOWEST)?;

        loop {
            match it.peek().unwrap_or(&EOF_TOKEN) {
                Token {
                    kind: EOF,
                    ref index,
                } => {
                    return Err(JSONPathError::syntax(
                        String::from("unbalanced parentheses"),
                        *index,
                    ));
                }
                Token { kind: RParen, .. } => break,
                _ => expr = self.parse_infix_expression(it, expr)?,
            }
        }

        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Some(Token { kind: RParen, .. })),
            "expected closing paren"
        );

        it.next(); // eat closing paren
        Ok(expr)
    }

    fn parse_basic_expression(&self, it: &mut Tokens) -> Result<FilterExpression, JSONPathError> {
        match it.peek().unwrap_or(&EOF_TOKEN) {
            Token {
                kind: DoubleQuoteString { value },
                index,
            } => {
                let value = unescape_string(value, index)?;
                let token = it.next().unwrap();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::String { value },
                ))
            }
            Token { kind: False, .. } => {
                let token = it.next().unwrap();
                Ok(FilterExpression::new(token, FilterExpressionType::False))
            }
            Token {
                kind: Float { ref value },
                index,
            } => {
                let f = value.parse::<f64>().map_err(|_| {
                    JSONPathError::syntax(String::from("invalid float literal"), *index)
                })?;
                let token = it.next().unwrap();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Float { value: f },
                ))
            }
            Token {
                kind: Function { .. },
                ..
            } => self.parse_function_call(it),
            Token {
                kind: Int { value },
                index,
            } => {
                let i = value.parse::<f64>().map_err(|_| {
                    JSONPathError::syntax(String::from("invalid float literal"), *index)
                })? as i64;
                let token = it.next().unwrap();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Int { value: i },
                ))
            }
            Token { kind: Null, .. } => {
                let token = it.next().unwrap();
                Ok(FilterExpression::new(token, FilterExpressionType::Null))
            }
            Token { kind: Root, .. } => {
                let token = it.next().unwrap();
                let segments = self.parse_segments(it)?;
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::RootQuery {
                        query: Box::new(Query { segments }),
                    },
                ))
            }
            Token { kind: Current, .. } => {
                let token = it.next().unwrap();
                let segments = self.parse_segments(it)?;
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::RelativeQuery {
                        query: Box::new(Query { segments }),
                    },
                ))
            }
            Token {
                kind: SingleQuoteString { value },
                index,
            } => {
                let value = unescape_string(&value.replace("\\'", "'"), index)?;
                let token = it.next().unwrap();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::String { value },
                ))
            }
            Token { kind: True, .. } => {
                let token = it.next().unwrap();
                Ok(FilterExpression::new(token, FilterExpressionType::True))
            }
            Token { kind: LParen, .. } => self.parse_grouped_expression(it),
            Token { kind: Not, .. } => self.parse_not_expression(it),
            Token { kind, index } => Err(JSONPathError::syntax(
                format!("unexpected basic expression token {}", kind),
                *index,
            )),
        }
    }

    fn parse_function_call(&self, it: &mut Tokens) -> Result<FilterExpression, JSONPathError> {
        let token = it.next().unwrap();
        let mut arguments: Vec<FilterExpression> = Vec::new();

        while it.peek().unwrap_or(&EOF_TOKEN).kind != RParen {
            let mut expr = self.parse_basic_expression(it)?;

            while matches!(
                it.peek().unwrap_or(&EOF_TOKEN).kind,
                Eq | Ge | Gt | Le | Lt | Ne | And | Or
            ) {
                expr = self.parse_infix_expression(it, expr)?
            }

            arguments.push(expr);

            match it.peek().unwrap_or(&EOF_TOKEN) {
                Token { kind: RParen, .. } => {
                    break;
                }
                Token { kind: Comma, .. } => {
                    it.next(); // eat comma
                }
                _ => {
                    ();
                }
            }
        }

        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Some(Token { kind: RParen, .. })),
            "expected closing paren"
        );

        it.next(); // eat closing paren

        if let Function { ref name } = &token.kind {
            let function_name = name.to_string();
            Ok(FilterExpression::new(
                token,
                FilterExpressionType::Function {
                    name: function_name,
                    args: arguments,
                },
            ))
        } else {
            Err(JSONPathError::syntax(
                format!("unexpected function argument token {}", token.kind),
                token.index,
            ))
        }
    }

    fn parse_filter_expression(
        &self,
        it: &mut Tokens,
        precedence: u8,
    ) -> Result<FilterExpression, JSONPathError> {
        let mut left = self.parse_basic_expression(it)?;

        loop {
            let peek_kind = &it.peek().unwrap_or(&EOF_TOKEN).kind;
            if matches!(peek_kind, EOF | RBracket)
                || self.precedence(peek_kind) < precedence
                || !matches!(peek_kind, Eq | Ge | Gt | Le | Lt | Ne | And | Or)
            {
                break;
            }

            left = self.parse_infix_expression(it, left)?;
        }

        Ok(left)
    }

    fn precedence(&self, kind: &TokenType) -> u8 {
        match kind {
            And => PRECEDENCE_LOGICAL_AND,
            Eq | Ge | Gt | Le | Lt | Ne => PRECEDENCE_RELATIONAL,
            Not => PRECEDENCE_LOGICAL_OR,
            Or => PRECEDENCE_LOGICAL_OR,
            _ => PRECEDENCE_LOWEST,
        }
    }
}

fn validate_index(value: &Box<str>, token_index: usize) -> Result<(), JSONPathError> {
    if value.len() > 1 && (value.starts_with("0") || value.starts_with("-0")) {
        Err(JSONPathError::syntax(
            format!("invalid index '{}'", value),
            token_index,
        ))
    } else {
        Ok(())
    }
}

fn unescape_string(value: &str, token_index: &usize) -> Result<String, JSONPathError> {
    let chars = value.chars().collect::<Vec<char>>();
    let length = chars.len();
    let mut rv = String::new();
    let mut index: usize = 0;

    while index < length {
        let start_index = token_index + index; // for error reporting

        match chars[index] {
            '\\' => {
                if index + 1 >= length {
                    return Err(JSONPathError::syntax(
                        String::from("invalid escape"),
                        start_index,
                    ));
                }

                index += 1;

                match chars[index] {
                    '"' => rv.push('"'),
                    '\\' => rv.push('\\'),
                    '/' => rv.push('/'),
                    'b' => rv.push('\x0C'),
                    'f' => rv.push('\x08'),
                    'n' => rv.push('\n'),
                    'r' => rv.push('\r'),
                    't' => rv.push('\t'),
                    'u' => {
                        // expect four hex digits
                        if index + 4 >= length {
                            return Err(JSONPathError::syntax(
                                String::from("invalid \\uXXXX escape"),
                                start_index,
                            ));
                        }

                        index += 1;

                        let digits = chars
                            .get(index..index + 4)
                            .unwrap()
                            .iter()
                            .collect::<String>();

                        let mut codepoint = u32::from_str_radix(&digits, 16).or_else(|_| {
                            Err(JSONPathError::syntax(
                                String::from("invalid \\uXXXX escape"),
                                start_index,
                            ))
                        })?;

                        if index + 5 < length && chars[index + 4] == '\\' && chars[index + 5] == 'u'
                        {
                            // expect a surrogate pair
                            if index + 9 >= length {
                                return Err(JSONPathError::syntax(
                                    String::from("invalid \\uXXXX escape"),
                                    start_index,
                                ));
                            }

                            let digits = &chars
                                .get(index + 6..index + 10)
                                .unwrap()
                                .iter()
                                .collect::<String>();

                            let low_surrogate = u32::from_str_radix(&digits, 16).or_else(|_| {
                                Err(JSONPathError::syntax(
                                    String::from("invalid \\uXXXX escape"),
                                    start_index,
                                ))
                            })?;

                            codepoint =
                                0x10000 + (((codepoint & 0x03FF) << 10) | (low_surrogate & 0x03FF));

                            index += 6;
                        }

                        let unescaped = char::from_u32(codepoint).ok_or_else(|| {
                            JSONPathError::syntax(
                                String::from("invalid \\uXXXX escape"),
                                start_index,
                            )
                        })?;

                        if unescaped as u32 <= 0x1F {
                            return Err(JSONPathError::syntax(
                                String::from("invalid character"),
                                start_index,
                            ));
                        }

                        rv.push(unescaped);
                        index += 3;
                    }
                    _ => {
                        return Err(JSONPathError::syntax(
                            String::from("invalid escape"),
                            start_index,
                        ));
                    }
                }
            }
            c => {
                if c as u32 <= 0x1F {
                    return Err(JSONPathError::syntax(
                        String::from("invalid character"),
                        start_index,
                    ));
                }
                rv.push(c);
            }
        }

        index += 1;
    }

    Ok(rv)
}
