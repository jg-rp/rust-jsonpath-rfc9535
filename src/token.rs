use core::fmt;

pub const EOQ: char = '\0';

#[derive(Debug, PartialEq, Clone)]
pub enum TokenType {
    Eoq,
    Error { msg: Box<str> },

    Colon,
    Comma,
    DoubleDot,
    Filter,
    Index { value: Box<str> },
    LBracket,
    Name { value: Box<str> },
    RBracket,
    Root,
    Wild,

    And,
    Current,
    DoubleQuoteString { value: Box<str> },
    Eq,
    False,
    Float { value: Box<str> },
    Function { name: Box<str> },
    Ge,
    Gt,
    Int { value: Box<str> },
    Le,
    LParen,
    Lt,
    Ne,
    Not,
    Null,
    Or,
    RParen,
    SingleQuoteString { value: Box<str> },
    True,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenType::Eoq => f.write_str("'end of query'"),
            TokenType::Error { msg } => write!(f, "error: {}", *msg),
            TokenType::Colon => f.write_str("';'"),
            TokenType::Comma => f.write_str("','"),
            TokenType::DoubleDot => f.write_str("'..'"),
            TokenType::Filter => f.write_str("'?'"),
            TokenType::Index { value } => write!(f, "'{}'", *value),
            TokenType::LBracket => f.write_str("'['"),
            TokenType::Name { value } => write!(f, "'{}'", *value),
            TokenType::RBracket => f.write_str("']'"),
            TokenType::Root => f.write_str("'$'"),
            TokenType::Wild => f.write_str("'*'"),
            TokenType::And => f.write_str("'&&'"),
            TokenType::Current => f.write_str("'@'"),
            TokenType::DoubleQuoteString { value } => write!(f, "'{}'", *value),
            TokenType::Eq => f.write_str("'=='"),
            TokenType::False => f.write_str("'false'"),
            TokenType::Float { value } => write!(f, "{}", *value),
            TokenType::Function { name } => write!(f, "'{}'", *name),
            TokenType::Ge => f.write_str("'>='"),
            TokenType::Gt => f.write_str("'>'"),
            TokenType::Int { value } => write!(f, "{}", *value),
            TokenType::Le => f.write_str("<='"),
            TokenType::LParen => f.write_str("'('"),
            TokenType::Lt => f.write_str("'<'"),
            TokenType::Ne => f.write_str("'!='"),
            TokenType::Not => f.write_str("'!'"),
            TokenType::Null => f.write_str("'null'"),
            TokenType::Or => f.write_str("'or'"),
            TokenType::RParen => f.write_str("')'"),
            TokenType::SingleQuoteString { value } => write!(f, "'{}'", *value),
            TokenType::True => f.write_str("'true'"),
        }
    }
}

// TODO: span?

/// A JSONPath expression token, as produced by the lexer.
#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenType,
    pub span: (usize, usize),
}

impl Token {
    pub fn new(kind: TokenType, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: (start, end),
        }
    }
}
