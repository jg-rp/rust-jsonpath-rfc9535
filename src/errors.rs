use std::fmt;

#[derive(Debug)]
pub enum JSONPathErrorType {
    LexerError,
    SyntaxError,
    TypeError,
}

#[derive(Debug)]
pub struct JSONPathError {
    pub error: JSONPathErrorType,
    pub msg: String, // TODO: or Box<str>
    pub index: usize,
}

impl JSONPathError {
    pub fn new(error: JSONPathErrorType, msg: String, index: usize) -> Self {
        Self { error, msg, index }
    }

    pub fn syntax(msg: String, index: usize) -> Self {
        Self {
            error: JSONPathErrorType::SyntaxError,
            msg,
            index,
        }
    }

    pub fn typ(msg: String, index: usize) -> Self {
        Self {
            error: JSONPathErrorType::TypeError,
            msg,
            index,
        }
    }
}

impl std::error::Error for JSONPathError {}

impl fmt::Display for JSONPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: move message prefix to Display for JSONPathErrorType
        match self.error {
            JSONPathErrorType::LexerError => {
                write!(f, "lexer error: {} ({})", self.msg, self.index)
            }
            JSONPathErrorType::SyntaxError => {
                write!(f, "syntax error: {} ({})", self.msg, self.index)
            }
            JSONPathErrorType::TypeError => {
                write!(f, "type error: {} ({})", self.msg, self.index)
            }
        }
    }
}
