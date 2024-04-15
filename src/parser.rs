use std::{iter::Peekable, vec::IntoIter};

use crate::{
    env::{Env, ExpressionType, FunctionSignature},
    errors::{JSONPathError, JSONPathErrorType},
    lexer::lex,
    query::{
        ComparisonOperator, FilterExpression, FilterExpressionType, LogicalOperator, Query,
        Segment, Selector,
    },
    token::{Token, TokenType},
};

use TokenType::*;

const EOF_TOKEN: Token = Token {
    kind: Eoq,
    index: 0, // change to usize max?
};

const PRECEDENCE_LOWEST: u8 = 1;
const PRECEDENCE_LOGICAL_OR: u8 = 3;
const PRECEDENCE_LOGICAL_AND: u8 = 4;
const PRECEDENCE_RELATIONAL: u8 = 5;
const PRECEDENCE_LOGICAL_NOT: u8 = 7;

struct TokenStream {
    tokens: Peekable<IntoIter<Token>>,
}

impl TokenStream {
    fn next(&mut self) -> Token {
        if let Some(token) = self.tokens.next() {
            token
        } else {
            EOF_TOKEN
        }
    }

    fn peek(&mut self) -> &Token {
        if let Some(token) = self.tokens.peek() {
            token
        } else {
            &EOF_TOKEN
        }
    }
}

pub struct Parser {
    pub env: Env,
}

impl Parser {
    pub fn new(env: Env) -> Self {
        Parser { env }
    }

    pub fn standard() -> Self {
        Parser {
            env: Env::standard(),
        }
    }

    pub fn add_function(
        &mut self,
        name: &str,
        params: Vec<ExpressionType>,
        returns: ExpressionType,
    ) {
        self.env.add_function(name, params, returns);
    }

    pub fn from_str(&self, query: &str) -> Result<Query, JSONPathError> {
        Ok(Query::new(self.parse(lex(query)?)?))
    }

    pub fn parse(&self, tokens: Vec<Token>) -> Result<Vec<Segment>, JSONPathError> {
        let mut it = TokenStream {
            tokens: tokens.into_iter().peekable(),
        };

        match it.next() {
            Token { kind: Root, .. } => {
                let segments = self.parse_segments(&mut it)?;
                // parse_segments should have consumed all tokens
                match it.next() {
                    Token { kind: Eoq, .. } => Ok(segments),
                    token => Err(JSONPathError::syntax(
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

    fn parse_segments(&self, it: &mut TokenStream) -> Result<Vec<Segment>, JSONPathError> {
        let mut segments: Vec<Segment> = Vec::new();
        loop {
            match it.peek().kind {
                DoubleDot => {
                    let token = it.next();
                    let selectors = self.parse_selectors(it)?;
                    segments.push(Segment::Recursive { token, selectors });
                }
                LBracket | Name { .. } | Wild => {
                    let token = (*it.peek()).clone();
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

    fn parse_selectors(&self, it: &mut TokenStream) -> Result<Vec<Selector>, JSONPathError> {
        match it.peek() {
            Token {
                kind: Name { value },
                index,
            } => {
                let name = unescape_string(value, index)?;
                let token = it.next();
                Ok(vec![Selector::Name { token, name }])
            }
            Token { kind: Wild, .. } => Ok(vec![Selector::Wild { token: it.next() }]),
            Token { kind: LBracket, .. } => self.parse_bracketed(it),
            _ => Ok(Vec::new()),
        }
    }

    fn parse_bracketed(&self, it: &mut TokenStream) -> Result<Vec<Selector>, JSONPathError> {
        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Token { kind: LBracket, .. }),
            "expected the start of a bracketed selection"
        );

        let token = it.next(); // LBracket
        let mut selectors: Vec<Selector> = Vec::new();

        loop {
            match it.peek() {
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
                    let name = unescape_string(value, index)?;
                    let token = it.next();
                    selectors.push(Selector::Name { token, name });
                }
                Token {
                    kind: SingleQuoteString { value },
                    index,
                } => {
                    let name = unescape_string(&value.replace("\\'", "'"), index)?;
                    let token = it.next();
                    selectors.push(Selector::Name { token, name });
                }
                Token { kind: Wild, .. } => {
                    let token = it.next();
                    selectors.push(Selector::Wild { token });
                }
                Token { kind: Filter, .. } => {
                    let selector = self.parse_filter(it)?;
                    selectors.push(selector);
                }
                Token { kind: Eoq, .. } => {
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
                    Token {
                        kind: Comma | TokenType::RBracket,
                        ..
                    }
                ),
                "expected a comma or the end of a bracketed selection"
            );

            // expect a comma or closing bracket
            match it.peek() {
                Token { kind: RBracket, .. } => continue,
                Token { kind: Comma, .. } => {
                    // eat comma
                    it.next();
                }
                token => {
                    return Err(JSONPathError::new(
                        JSONPathErrorType::SyntaxError,
                        format!("expected a comma or closing bracket, found {}", token.kind),
                        token.index,
                    ));
                }
            }
        }

        if selectors.is_empty() {
            return Err(JSONPathError::new(
                JSONPathErrorType::SyntaxError,
                String::from("empty bracketed selection"),
                token.index,
            ));
        }

        Ok(selectors)
    }

    fn parse_slice_or_index(&self, it: &mut TokenStream) -> Result<Selector, JSONPathError> {
        let token = it.next(); // index or colon

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

        if token.kind == Colon || it.peek().kind == Colon {
            // a slice
            let mut start: Option<i64> = None;
            let mut stop: Option<i64> = None;
            let mut step: Option<i64> = None;

            // 1: or :
            if let Token {
                kind: Index { ref value },
                index,
            } = &token
            {
                start = Some(self.parse_i_json_int(value, *index)?);
                it.next(); // eat colon
            }

            // 1 or 1: or : or ?
            if matches!(it.peek().kind, Index { .. } | Colon) {
                if let Token {
                    kind: Index { ref value },
                    index,
                } = it.next()
                {
                    stop = Some(self.parse_i_json_int(value, index)?);
                    if it.peek().kind == Colon {
                        it.next(); // eat colon
                    }
                }
            }

            // 1 or ?
            if matches!(it.peek().kind, Index { .. }) {
                if let Token {
                    kind: Index { ref value },
                    index,
                } = it.next()
                {
                    step = Some(self.parse_i_json_int(value, index)?);
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
                    let array_index = self.parse_i_json_int(value, token.index)?;
                    Ok(Selector::Index {
                        token,
                        index: array_index,
                    })
                }
                tok => Err(JSONPathError::syntax(
                    format!("expected an index, found {}", tok.kind),
                    tok.index,
                )),
            }
        }
    }

    fn parse_filter(&self, it: &mut TokenStream) -> Result<Selector, JSONPathError> {
        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Token { kind: Filter, .. }),
            "expected a filter"
        );

        let token = it.next();
        let expr = self.parse_filter_expression(it, PRECEDENCE_LOWEST)?;

        if let FilterExpression {
            kind: FilterExpressionType::Function { ref name, .. },
            ..
        } = expr
        {
            if let Some(FunctionSignature {
                return_type: ExpressionType::Value,
                ..
            }) = self.env.functions.get(name)
            {
                return Err(JSONPathError::typ(
                    format!("result of {}() must be compared", name),
                    expr.token.index,
                ));
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

    fn parse_not_expression(
        &self,
        it: &mut TokenStream,
    ) -> Result<FilterExpression, JSONPathError> {
        let token = it.next();
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
        it: &mut TokenStream,
        left: FilterExpression,
    ) -> Result<FilterExpression, JSONPathError> {
        let op_token = it.next();
        let precedence = self.precedence(&op_token.kind);
        let right = self.parse_filter_expression(it, precedence)?;

        match op_token.kind {
            And => {
                if left.is_literal() || right.is_literal() {
                    Err(JSONPathError::syntax(
                        String::from("filter expression literals must be compared"),
                        left.token.index,
                    ))
                } else {
                    Ok(FilterExpression::new(
                        left.token.clone(),
                        FilterExpressionType::Logical {
                            left: Box::new(left),
                            operator: LogicalOperator::And,
                            right: Box::new(right),
                        },
                    ))
                }
            }
            Or => {
                if left.is_literal() || right.is_literal() {
                    Err(JSONPathError::syntax(
                        String::from("filter expression literals must be compared"),
                        left.token.index,
                    ))
                } else {
                    Ok(FilterExpression::new(
                        left.token.clone(),
                        FilterExpressionType::Logical {
                            left: Box::new(left),
                            operator: LogicalOperator::Or,
                            right: Box::new(right),
                        },
                    ))
                }
            }
            Eq => {
                self.assert_comparable(&left, left.token.index)?;
                self.assert_comparable(&right, right.token.index)?;
                Ok(FilterExpression::new(
                    left.token.clone(),
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Eq,
                        right: Box::new(right),
                    },
                ))
            }
            Ge => {
                self.assert_comparable(&left, left.token.index)?;
                self.assert_comparable(&right, right.token.index)?;
                Ok(FilterExpression::new(
                    left.token.clone(),
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Ge,
                        right: Box::new(right),
                    },
                ))
            }
            Gt => {
                self.assert_comparable(&left, left.token.index)?;
                self.assert_comparable(&right, right.token.index)?;
                Ok(FilterExpression::new(
                    left.token.clone(),
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Gt,
                        right: Box::new(right),
                    },
                ))
            }
            Le => {
                self.assert_comparable(&left, left.token.index)?;
                self.assert_comparable(&right, right.token.index)?;
                Ok(FilterExpression::new(
                    left.token.clone(),
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Le,
                        right: Box::new(right),
                    },
                ))
            }
            Lt => {
                self.assert_comparable(&left, left.token.index)?;
                self.assert_comparable(&right, right.token.index)?;
                Ok(FilterExpression::new(
                    left.token.clone(),
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Lt,
                        right: Box::new(right),
                    },
                ))
            }
            Ne => {
                self.assert_comparable(&left, left.token.index)?;
                self.assert_comparable(&right, right.token.index)?;
                Ok(FilterExpression::new(
                    left.token.clone(),
                    FilterExpressionType::Comparison {
                        left: Box::new(left),
                        operator: ComparisonOperator::Ne,
                        right: Box::new(right),
                    },
                ))
            }
            _ => Err(JSONPathError::syntax(
                format!("unexpected infix operator {}", op_token.kind),
                op_token.index,
            )),
        }
    }

    fn parse_grouped_expression(
        &self,
        it: &mut TokenStream,
    ) -> Result<FilterExpression, JSONPathError> {
        it.next(); // eat open paren
        let mut expr = self.parse_filter_expression(it, PRECEDENCE_LOWEST)?;

        loop {
            match it.peek() {
                Token {
                    kind: Eoq,
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
            matches!(it.peek(), Token { kind: RParen, .. }),
            "expected closing paren"
        );

        it.next(); // eat closing paren
        Ok(expr)
    }

    fn parse_basic_expression(
        &self,
        it: &mut TokenStream,
    ) -> Result<FilterExpression, JSONPathError> {
        match it.peek() {
            Token {
                kind: DoubleQuoteString { value },
                index,
            } => {
                let value = unescape_string(value, index)?;
                let token = it.next();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::String { value },
                ))
            }
            Token { kind: False, .. } => {
                let token = it.next();
                Ok(FilterExpression::new(token, FilterExpressionType::False))
            }
            Token {
                kind: Float { ref value },
                index,
            } => {
                let f = value.parse::<f64>().map_err(|_| {
                    JSONPathError::syntax(String::from("invalid float literal"), *index)
                })?;
                let token = it.next();
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
                    JSONPathError::syntax(String::from("invalid integer literal"), *index)
                })? as i64;

                let token = it.next();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::Int { value: i },
                ))
            }
            Token { kind: Null, .. } => {
                let token = it.next();
                Ok(FilterExpression::new(token, FilterExpressionType::Null))
            }
            Token { kind: Root, .. } => {
                let token = it.next();
                let segments = self.parse_segments(it)?;
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::RootQuery {
                        query: Box::new(Query { segments }),
                    },
                ))
            }
            Token { kind: Current, .. } => {
                let token = it.next();
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
                let token = it.next();
                Ok(FilterExpression::new(
                    token,
                    FilterExpressionType::String { value },
                ))
            }
            Token { kind: True, .. } => {
                let token = it.next();
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

    fn parse_function_call(&self, it: &mut TokenStream) -> Result<FilterExpression, JSONPathError> {
        let token = it.next();
        let mut arguments: Vec<FilterExpression> = Vec::new();

        while it.peek().kind != RParen {
            let mut expr = self.parse_basic_expression(it)?;

            while matches!(it.peek().kind, Eq | Ge | Gt | Le | Lt | Ne | And | Or) {
                expr = self.parse_infix_expression(it, expr)?
            }

            arguments.push(expr);

            match it.peek() {
                Token { kind: RParen, .. } => {
                    break;
                }
                Token { kind: Comma, .. } => {
                    it.next(); // eat comma
                }
                _ => (),
            }
        }

        #[cfg(debug_assertions)]
        debug_assert!(
            matches!(it.peek(), Token { kind: RParen, .. }),
            "expected closing paren"
        );

        it.next(); // eat closing paren

        if let Function { ref name } = &token.kind {
            let function_name = name.to_string();
            self.assert_well_typed(&function_name, &arguments, &token)?;
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
        it: &mut TokenStream,
        precedence: u8,
    ) -> Result<FilterExpression, JSONPathError> {
        let mut left = self.parse_basic_expression(it)?;

        loop {
            let peek_kind = &it.peek().kind;
            if matches!(peek_kind, Eoq | RBracket)
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
            Not => PRECEDENCE_LOGICAL_NOT,
            Or => PRECEDENCE_LOGICAL_OR,
            _ => PRECEDENCE_LOWEST,
        }
    }

    fn assert_comparable(
        &self,
        expr: &FilterExpression,
        index: usize,
    ) -> Result<(), JSONPathError> {
        match &expr.kind {
            FilterExpressionType::RelativeQuery { query }
            | FilterExpressionType::RootQuery { query } => {
                if !query.is_singular() {
                    Err(JSONPathError::typ(
                        String::from("non-singular query is not comparable"),
                        index,
                    ))
                } else {
                    Ok(())
                }
            }
            FilterExpressionType::Function { name, .. } => {
                if let Some(FunctionSignature {
                    return_type: ExpressionType::Value,
                    ..
                }) = self.env.functions.get(name)
                {
                    Ok(())
                } else {
                    Err(JSONPathError::typ(
                        format!("result of {}() is not comparable", name),
                        index,
                    ))
                }
            }
            _ => Ok(()),
        }
    }

    fn assert_well_typed(
        &self,
        func_name: &str,
        args: &[FilterExpression],
        token: &Token,
    ) -> Result<(), JSONPathError> {
        let signature = self.env.functions.get(func_name).ok_or_else(|| {
            JSONPathError::name(format!("unknown function '{}'", func_name), token.index)
        })?;

        // correct number of arguments?
        if args.len() != signature.param_types.len() {
            return Err(JSONPathError::typ(
                format!(
                    "{}() takes {} argument{} but {} were given",
                    func_name,
                    signature.param_types.len(),
                    if signature.param_types.len() > 1 {
                        "s"
                    } else {
                        ""
                    },
                    args.len()
                ),
                token.index,
            ));
        }

        // correct argument types?
        for (idx, typ) in signature.param_types.iter().enumerate() {
            let arg = &args[idx];
            match typ {
                ExpressionType::Value => {
                    if !self.is_value_type(arg) {
                        return Err(JSONPathError::typ(
                            format!(
                                "argument {} of {}() must be of a 'Value' type",
                                idx + 1,
                                func_name
                            ),
                            token.index,
                        ));
                    }
                }
                ExpressionType::Logical => {
                    if !matches!(
                        arg,
                        FilterExpression {
                            kind: FilterExpressionType::RelativeQuery { .. }
                                | FilterExpressionType::RootQuery { .. }
                                | FilterExpressionType::Logical { .. }
                                | FilterExpressionType::Comparison { .. },
                            ..
                        }
                    ) {
                        return Err(JSONPathError::typ(
                            format!(
                                "argument {} of {}() must be of a 'Logical' type",
                                idx + 1,
                                func_name
                            ),
                            token.index,
                        ));
                    }
                }
                ExpressionType::Nodes => {
                    if !self.is_nodes_type(arg) {
                        return Err(JSONPathError::typ(
                            format!(
                                "argument {} of {}() must be of a 'Nodes' type",
                                idx + 1,
                                func_name
                            ),
                            token.index,
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn is_value_type(&self, expr: &FilterExpression) -> bool {
        // literals are values
        if expr.is_literal() {
            return true;
        }

        // singular queries will be coerced to a value
        if let FilterExpression {
            kind:
                FilterExpressionType::RelativeQuery { query }
                | FilterExpressionType::RootQuery { query },
            ..
        } = expr
        {
            if query.is_singular() {
                return true;
            }
        }

        // some functions return a value
        if let FilterExpression {
            kind: FilterExpressionType::Function { name, .. },
            ..
        } = expr
        {
            if let Some(FunctionSignature {
                return_type: ExpressionType::Value,
                ..
            }) = self.env.functions.get(name)
            {
                return true;
            }
        }

        false
    }

    fn is_nodes_type(&self, expr: &FilterExpression) -> bool {
        if matches!(
            expr,
            FilterExpression {
                kind: FilterExpressionType::RelativeQuery { .. }
                    | FilterExpressionType::RootQuery { .. },
                ..
            }
        ) {
            return true;
        }

        if let FilterExpression {
            kind: FilterExpressionType::Function { name, .. },
            ..
        } = expr
        {
            if let Some(FunctionSignature {
                return_type: ExpressionType::Nodes,
                ..
            }) = self.env.functions.get(name)
            {
                return true;
            }
        }

        false
    }

    fn parse_i_json_int(&self, value: &str, token_index: usize) -> Result<i64, JSONPathError> {
        if value.len() > 1 && (value.starts_with('0') || value.starts_with("-0")) {
            return Err(JSONPathError::syntax(
                format!("invalid index '{}'", value),
                token_index,
            ));
        }

        let index = value.parse::<i64>().map_err(|_| {
            JSONPathError::syntax(format!("invalid index '{}'", value), token_index)
        })?;

        if !self.env.index_range.contains(&index) {
            return Err(JSONPathError::syntax(
                format!("index out of range '{}'", value),
                token_index,
            ));
        }

        Ok(index)
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

                        let mut codepoint = u32::from_str_radix(&digits, 16).map_err(|_| {
                            JSONPathError::syntax(
                                String::from("invalid \\uXXXX escape"),
                                start_index,
                            )
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

                            let low_surrogate = u32::from_str_radix(digits, 16).map_err(|_| {
                                JSONPathError::syntax(
                                    String::from("invalid \\uXXXX escape"),
                                    start_index,
                                )
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
