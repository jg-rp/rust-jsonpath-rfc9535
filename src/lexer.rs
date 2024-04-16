// TODO: docs
use crate::{
    errors::JSONPathError,
    token::{Token, TokenType, EOQ},
};

use std::str::CharIndices;

enum State {
    Error,
    EndOfQuery,
    LexRoot,
    LexSegment,
    LexDescendantSegment,
    LexShorthandSegment,
    LexInsideBracketedSegment,
    LexInsideFilter,
    LexInsideSingleQuotedString,
    LexInsideDoubleQuotedString,
    LexInsideSingleQuotedFilterString,
    LexInsideDoubleQuotedFilterString,
}

/// A JSONPath tokenizer, producing a vector of tokens.
struct Lexer<'q> {
    query: &'q str,
    tokens: Vec<Token>,

    chars: CharIndices<'q>,
    start: usize,
    pos: usize,

    filter_depth: u32,
    paren_stack: Vec<u32>,
}

impl<'q> Lexer<'q> {
    fn new(query: &'q str) -> Self {
        Self {
            query,
            tokens: Vec::new(),
            start: 0,
            pos: 0,
            chars: query.char_indices(),
            filter_depth: 0,
            paren_stack: Vec::new(),
        }
    }

    fn run(&mut self) {
        let mut state = State::LexRoot;
        loop {
            match state {
                State::Error | State::EndOfQuery => break,
                State::LexRoot => state = lex_root(self),
                State::LexSegment => state = lex_segment(self),
                State::LexDescendantSegment => state = lex_descendant_segment(self),
                State::LexShorthandSegment => state = lex_shorthand_selector(self),
                State::LexInsideBracketedSegment => state = lex_inside_bracketed_segment(self),
                State::LexInsideFilter => state = lex_inside_filter(self),
                State::LexInsideSingleQuotedString => {
                    state = lex_string(self, '\'', State::LexInsideBracketedSegment)
                }
                State::LexInsideDoubleQuotedString => {
                    state = lex_string(self, '"', State::LexInsideBracketedSegment)
                }
                State::LexInsideSingleQuotedFilterString => {
                    state = lex_string(self, '\'', State::LexInsideFilter)
                }
                State::LexInsideDoubleQuotedFilterString => {
                    state = lex_string(self, '"', State::LexInsideFilter)
                }
            }
        }
    }

    fn emit(&mut self, t: TokenType) {
        self.tokens.push(Token::new(t, self.start, self.pos));
        self.start = self.pos;
    }

    fn value(&self) -> &str {
        self.query
            .get(self.start..self.pos)
            .expect("lexer error: slice out of bounds or not on codepoint boundary")
    }

    fn boxed_value(&self) -> Box<str> {
        self.value().to_string().into_boxed_str()
    }

    fn next(&mut self) -> Option<char> {
        if let Some((pos, ch)) = self.chars.next() {
            self.pos = pos + ch.len_utf8();

            #[cfg(debug_assertions)]
            debug_assert!(
                self.pos <= self.query.len(),
                "current position is out of bounds"
            );

            Some(ch)
        } else {
            None
        }
    }

    fn ignore(&mut self) {
        self.start = self.pos;
    }

    fn peek(&mut self) -> char {
        if let Some((_, ch)) = self.chars.clone().next() {
            ch
        } else {
            EOQ
        }
    }

    fn accept(&mut self, ch: char) -> bool {
        if self.peek() == ch {
            self.next();
            true
        } else {
            false
        }
    }

    fn accept_if(&mut self, pred: impl FnOnce(char) -> bool) -> bool {
        if pred(self.peek()) {
            self.next();
            true
        } else {
            false
        }
    }

    fn accept_run(&mut self, pred: impl Fn(char) -> bool) -> bool {
        let mut accepted = false;
        while pred(self.peek()) {
            self.next();
            accepted = true;
        }
        accepted
    }

    fn ignore_whitespace(&mut self) -> bool {
        #[cfg(debug_assertions)]
        debug_assert!(
            self.pos == self.start,
            "must emit or ignore before eating whitespace"
        );

        if self.accept_run(is_whitespace_char) {
            self.ignore();
            true
        } else {
            false
        }
    }

    fn error(&mut self, msg: String) -> State {
        self.tokens.push(Token::new(
            TokenType::Error {
                msg: msg.into_boxed_str(),
            },
            self.start,
            self.pos,
        ));
        State::Error
    }
}

pub fn tokenize(query: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(query);
    lexer.run();
    lexer.tokens
}

pub fn lex(query: &str) -> Result<Vec<Token>, JSONPathError> {
    let tokens = tokenize(query);

    match tokens.last() {
        Some(Token {
            kind: TokenType::Error { msg },
            span,
            ..
        }) => Err(JSONPathError::syntax((*msg).to_string(), *span)),
        _ => Ok(tokens),
    }
}

fn lex_root(l: &mut Lexer) -> State {
    if l.accept('$') {
        l.emit(TokenType::Root);
        State::LexSegment
    } else {
        let msg = format!("expected '$', found '{}'", l.next().unwrap_or(EOQ));
        l.error(msg)
    }
}

fn lex_segment(l: &mut Lexer) -> State {
    if l.ignore_whitespace() && l.peek() == EOQ {
        return l.error(String::from("unexpected trailing whitespace"));
    }

    if l.accept('.') {
        if l.accept('.') {
            l.emit(TokenType::DoubleDot);
            State::LexDescendantSegment
        } else {
            State::LexShorthandSegment
        }
    } else if l.accept('[') {
        l.emit(TokenType::LBracket);
        State::LexInsideBracketedSegment
    } else if l.filter_depth > 0 {
        State::LexInsideFilter
    } else if l.peek() == EOQ {
        l.next();
        l.emit(TokenType::Eoq);
        State::EndOfQuery
    } else {
        let msg = format!(
            "expected '.', '..' or a bracketed selection, found '{}'",
            l.next().unwrap_or(EOQ)
        );
        l.error(msg)
    }
}

fn lex_descendant_segment(l: &mut Lexer) -> State {
    if l.accept('*') {
        l.emit(TokenType::Wild);
        State::LexSegment
    } else if l.accept('[') {
        l.emit(TokenType::LBracket);
        State::LexInsideBracketedSegment
    } else if l.accept_if(is_name_first) {
        l.accept_run(is_name_char);
        l.emit(TokenType::Name {
            value: l.boxed_value(),
        });
        State::LexSegment
    } else {
        let msg = format!("unexpected descendant selection token '{}'", l.peek());
        l.error(msg)
    }
}

fn lex_shorthand_selector(l: &mut Lexer) -> State {
    l.ignore(); // ignore dot

    if l.accept_run(is_whitespace_char) {
        return l.error(String::from("unexpected whitespace after dot"));
    }

    if l.accept('*') {
        l.emit(TokenType::Wild);
        State::LexSegment
    } else if l.accept_if(is_name_first) {
        l.accept_run(is_name_char);
        l.emit(TokenType::Name {
            value: l.boxed_value(),
        });
        State::LexSegment
    } else {
        let msg = format!(
            "unexpected shorthand selector '{}'",
            l.next().unwrap_or(EOQ)
        );
        l.error(msg)
    }
}

fn lex_inside_bracketed_segment(l: &mut Lexer) -> State {
    l.ignore_whitespace();

    match l.peek() {
        ']' => {
            l.next();
            l.emit(TokenType::RBracket);
            if l.filter_depth > 0 {
                State::LexInsideFilter
            } else {
                State::LexSegment
            }
        }
        '*' => {
            l.next();
            l.emit(TokenType::Wild);
            State::LexInsideBracketedSegment
        }
        '?' => {
            l.next();
            l.emit(TokenType::Filter);
            l.filter_depth += 1;
            State::LexInsideFilter
        }
        ',' => {
            l.next();
            l.emit(TokenType::Comma);
            State::LexInsideBracketedSegment
        }
        ':' => {
            l.next();
            l.emit(TokenType::Colon);
            State::LexInsideBracketedSegment
        }
        '\'' => {
            l.next();
            State::LexInsideSingleQuotedString
        }
        '"' => {
            l.next();
            State::LexInsideDoubleQuotedString
        }
        '-' => {
            // negative array index or slice
            l.next();
            if l.accept_run(is_digit) {
                l.emit(TokenType::Index {
                    value: l.boxed_value(),
                });
                State::LexInsideBracketedSegment
            } else {
                let msg = format!("expected a digit after '-', found '{}'", l.peek());
                l.error(msg)
            }
        }
        EOQ => l.error(String::from("unclosed bracketed selection")),
        _ => {
            if l.accept_run(is_digit) {
                l.emit(TokenType::Index {
                    value: l.boxed_value(),
                });
                State::LexInsideBracketedSegment
            } else {
                let msg = format!("unexpected '{}' in bracketed selection", l.peek());
                l.error(msg)
            }
        }
    }
}

fn lex_inside_filter(l: &mut Lexer) -> State {
    l.ignore_whitespace();

    match l.peek() {
        EOQ => l.error(String::from("unclosed bracketed selection")),
        ']' => {
            l.filter_depth -= 1;
            if l.paren_stack.len() == 1 {
                l.error(String::from("unbalanced parentheses"))
            } else {
                State::LexInsideBracketedSegment
            }
        }
        ',' => {
            l.next();
            l.emit(TokenType::Comma);
            // If we have unbalanced parens, we are inside a function call and a
            // comma separates arguments. Otherwise a comma separates selectors.
            if !l.paren_stack.is_empty() {
                State::LexInsideFilter
            } else {
                l.filter_depth -= 1;
                State::LexInsideBracketedSegment
            }
        }
        '\'' => {
            l.next();
            State::LexInsideSingleQuotedFilterString
        }
        '"' => {
            l.next();
            State::LexInsideDoubleQuotedFilterString
        }
        '(' => {
            l.next();
            l.emit(TokenType::LParen);
            // Are we in a function call? If so, a function argument contains parens.
            if let Some(i) = l.paren_stack.last_mut() {
                *i += 1;
            }
            State::LexInsideFilter
        }
        ')' => {
            l.next();
            l.emit(TokenType::RParen);
            // Are we closing a function call or a parenthesized expression?
            if !l.paren_stack.is_empty() {
                if *l.paren_stack.last().unwrap() == 1 {
                    l.paren_stack.pop();
                } else {
                    *l.paren_stack.last_mut().unwrap() -= 1;
                }
            }
            State::LexInsideFilter
        }
        '$' => {
            l.next();
            l.emit(TokenType::Root);
            State::LexSegment
        }
        '@' => {
            l.next();
            l.emit(TokenType::Current);
            State::LexSegment
        }
        '.' => State::LexSegment,
        '!' => {
            l.next();
            if l.accept('=') {
                l.emit(TokenType::Ne);
            } else {
                l.emit(TokenType::Not);
            }
            State::LexInsideFilter
        }
        '=' => {
            l.next();
            if l.accept('=') {
                l.emit(TokenType::Eq);
            } else {
                return l.error(String::from("expected '==', found '='"));
            }
            State::LexInsideFilter
        }
        '<' => {
            l.next();
            if l.accept('=') {
                l.emit(TokenType::Le);
            } else {
                l.emit(TokenType::Lt);
            }
            State::LexInsideFilter
        }
        '>' => {
            l.next();
            if l.accept('=') {
                l.emit(TokenType::Ge);
            } else {
                l.emit(TokenType::Gt);
            }
            State::LexInsideFilter
        }
        '&' => {
            l.next();
            if l.accept('&') {
                l.emit(TokenType::And);
            } else {
                return l.error(String::from("unexpected '&', did you mean '&&'?"));
            }
            State::LexInsideFilter
        }
        '|' => {
            l.next();
            if l.accept('|') {
                l.emit(TokenType::Or);
            } else {
                return l.error(String::from("unexpected '|', did you mean '||'?"));
            }
            State::LexInsideFilter
        }
        '-' => {
            // negative number
            l.next();
            lex_number(l)
        }
        _ => {
            if is_digit(l.peek()) {
                // positive number
                lex_number(l);
            } else if l.accept_run(is_function_name_first) {
                // function name or keyword
                l.accept_run(is_function_name_char);
                match l.value() {
                    "true" => l.emit(TokenType::True),
                    "false" => l.emit(TokenType::False),
                    "null" => l.emit(TokenType::Null),
                    _ => {
                        if l.peek() == '(' {
                            // a function call
                            l.paren_stack.push(1);
                            l.emit(TokenType::Function {
                                name: l.boxed_value(),
                            });
                            l.next();
                            l.ignore(); // discard the left paren
                        } else {
                            return l.error(String::from("expected a keyword or function call"));
                        }
                    }
                }
            } else {
                let msg = format!("unexpected filter expression token '{}'", l.peek());
                return l.error(msg);
            }

            State::LexInsideFilter
        }
    }
}

fn lex_string(l: &mut Lexer, quote: char, next_state: State) -> State {
    l.ignore(); // ignore open quote

    if l.peek() == EOQ {
        todo!("handle end of query after open quote");
    }

    loop {
        match l.peek() {
            '\\' => {
                l.next();
                if !l.accept_if(|c| is_escape_char(c) || c == quote) {
                    return l.error(String::from("invalid escape sequence"));
                }
            }
            EOQ => {
                let msg = format!("unclosed string starting at index {}", l.start);
                return l.error(msg);
            }
            ch => {
                if ch == quote {
                    l.emit(match quote {
                        '\'' => TokenType::SingleQuoteString {
                            value: l.boxed_value(),
                        },
                        '"' => TokenType::DoubleQuoteString {
                            value: l.boxed_value(),
                        },
                        _ => panic!("unexpected quote delimiter '{}'", quote),
                    });
                    l.next();
                    l.ignore(); // ignore closing quote
                    return next_state;
                }
                l.next();
            }
        }
    }
}

fn lex_number(l: &mut Lexer) -> State {
    if !l.accept_run(is_digit) {
        let msg = format!("expected a digit, found '{}'", l.peek());
        return l.error(msg);
    }

    if l.accept('.') {
        // a float
        if !l.accept_run(is_digit) {
            return l.error(String::from(
                "a fractional digit is required after a decimal point",
            ));
        }

        // exponent
        if l.accept('e') {
            l.accept_if(|ch| ch == '+' || ch == '-');
            if !l.accept_run(is_digit) {
                return l.error(String::from("at least one exponent digit is required"));
            }
        }

        l.emit(TokenType::Float {
            value: l.boxed_value(),
        });
    } else {
        // exponent
        if l.accept('e') {
            if l.accept('-') {
                // emit a float if exponent is negative
                if !l.accept_run(is_digit) {
                    return l.error(String::from("at least one exponent digit is required"));
                }
                l.emit(TokenType::Float {
                    value: l.boxed_value(),
                });
            } else {
                l.accept('+');
                if !l.accept_run(is_digit) {
                    return l.error(String::from("at least one exponent digit is required"));
                }
                l.emit(TokenType::Int {
                    value: l.boxed_value(),
                })
            }
        } else {
            l.emit(TokenType::Int {
                value: l.boxed_value(),
            })
        }
    }

    State::LexInsideFilter
}

fn is_name_first(ch: char) -> bool {
    let code_point = ch as u32;
    // surrogate pair code points are not representable with char
    (0x41..=0x5A).contains(&code_point)
        || code_point == 0x5F
        || (0x61..=0x7A).contains(&code_point)
        || code_point >= 0x80
}

fn is_name_char(ch: char) -> bool {
    let code_point = ch as u32;
    // surrogate pair code points are not representable with char
    (0x30..=0x39).contains(&code_point)
        || (0x41..=0x5A).contains(&code_point)
        || code_point == 0x5F
        || (0x61..=0x7A).contains(&code_point)
        || code_point >= 0x80
}

fn is_digit(ch: char) -> bool {
    // 0-9
    let code_point = ch as u32;
    (0x30..=0x39).contains(&code_point)
}

fn is_function_name_first(ch: char) -> bool {
    // a-z
    let code_point = ch as u32;
    (0x61..=0x7a).contains(&code_point)
}

fn is_function_name_char(ch: char) -> bool {
    // a-z 0-9 _
    let code_point = ch as u32;
    (0x30..=0x39).contains(&code_point) || code_point == 0x5F || (0x61..=0x7a).contains(&code_point)
}

fn is_escape_char(ch: char) -> bool {
    matches!(ch, 'b' | 'f' | 'n' | 'r' | 't' | 'u' | '/' | '\\')
}

fn is_whitespace_char(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\r' | '\t')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_shorthand_name() {
        let query = "$.foo.bar";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    6,
                    9
                ),
                Token::new(TokenType::Eoq, 9, 9),
            ]
        )
    }

    #[test]
    fn bracketed_name() {
        let query = "$['foo']['bar']";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(
                    TokenType::SingleQuoteString {
                        value: "foo".to_string().into_boxed_str()
                    },
                    3,
                    6
                ),
                Token::new(TokenType::RBracket, 7, 8),
                Token::new(TokenType::LBracket, 8, 9),
                Token::new(
                    TokenType::SingleQuoteString {
                        value: "bar".to_string().into_boxed_str()
                    },
                    10,
                    13
                ),
                Token::new(TokenType::RBracket, 14, 15),
                Token::new(TokenType::Eoq, 15, 15),
            ]
        )
    }

    #[test]
    fn basic_index() {
        let query = "$.foo[1]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5,
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::Index {
                        value: "1".to_string().into_boxed_str()
                    },
                    6,
                    7
                ),
                Token::new(TokenType::RBracket, 7, 8),
                Token::new(TokenType::Eoq, 8, 8),
            ]
        )
    }

    #[test]
    fn negative_index() {
        let query = "$.foo[-1]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::Index {
                        value: "-1".to_string().into_boxed_str()
                    },
                    6,
                    8
                ),
                Token::new(TokenType::RBracket, 8, 9),
                Token::new(TokenType::Eoq, 9, 9),
            ]
        )
    }

    #[test]
    fn just_a_hyphen() {
        let query = "$.foo[-]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::Error {
                        msg: "expected a digit after '-', found ']'"
                            .to_string()
                            .into_boxed_str()
                    },
                    6,
                    7
                ),
            ]
        )
    }

    #[test]
    fn missing_root_selector() {
        let query = "foo.bar";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![Token::new(
                TokenType::Error {
                    msg: "expected '$', found 'f'".to_string().into_boxed_str()
                },
                0,
                1
            ),]
        )
    }

    #[test]
    fn root_property_selector_without_dot() {
        let query = "$foo";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Error {
                        msg: "expected '.', '..' or a bracketed selection, found 'f'"
                            .to_string()
                            .into_boxed_str()
                    },
                    1,
                    2
                ),
            ]
        )
    }

    #[test]
    fn whitespace_after_root() {
        let query = "$ .foo.bar";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    3,
                    6
                ),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    7,
                    10
                ),
                Token::new(TokenType::Eoq, 10, 10),
            ]
        )
    }

    #[test]
    fn whitespace_before_dot_property() {
        let query = "$. foo.bar";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Error {
                        msg: "unexpected whitespace after dot"
                            .to_string()
                            .into_boxed_str()
                    },
                    2,
                    3
                ),
            ]
        )
    }

    #[test]
    fn whitespace_after_dot_property() {
        let query = "$.foo .bar";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    7,
                    10
                ),
                Token::new(TokenType::Eoq, 10, 10),
            ]
        )
    }

    #[test]
    fn basic_dot_wild() {
        let query = "$.foo.*";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::Wild, 6, 7),
                Token::new(TokenType::Eoq, 7, 7),
            ]
        )
    }

    #[test]
    fn recurse_name_shorthand() {
        let query = "$..foo";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::DoubleDot, 1, 3),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    3,
                    6
                ),
                Token::new(TokenType::Eoq, 6, 6),
            ]
        )
    }

    #[test]
    fn recurse_name_bracketed() {
        let query = "$..['foo']";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::DoubleDot, 1, 3),
                Token::new(TokenType::LBracket, 3, 4),
                Token::new(
                    TokenType::SingleQuoteString {
                        value: "foo".to_string().into_boxed_str()
                    },
                    5,
                    8
                ),
                Token::new(TokenType::RBracket, 9, 10),
                Token::new(TokenType::Eoq, 10, 10),
            ]
        )
    }

    #[test]
    fn recurse_wild_shorthand() {
        let query = "$..*";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::DoubleDot, 1, 3),
                Token::new(TokenType::Wild, 3, 4),
                Token::new(TokenType::Eoq, 4, 4),
            ]
        )
    }

    #[test]
    fn basic_recurse_with_trailing_dot() {
        let query = "$...foo";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::DoubleDot, 1, 3),
                Token::new(
                    TokenType::Error {
                        msg: "unexpected descendant selection token '.'"
                            .to_string()
                            .into_boxed_str()
                    },
                    3,
                    3
                ),
            ]
        )
    }

    #[test]
    fn erroneous_double_recurse() {
        let query = "$....foo";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::DoubleDot, 1, 3),
                Token::new(
                    TokenType::Error {
                        msg: "unexpected descendant selection token '.'"
                            .to_string()
                            .into_boxed_str()
                    },
                    3,
                    3
                ),
            ]
        )
    }

    #[test]
    fn bracketed_name_selector_double_quotes() {
        let query = "$.foo[\"bar\"]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::DoubleQuoteString {
                        value: "bar".to_string().into_boxed_str()
                    },
                    7,
                    10
                ),
                Token::new(TokenType::RBracket, 11, 12),
                Token::new(TokenType::Eoq, 12, 12),
            ]
        )
    }

    #[test]
    fn bracketed_name_selector_single_quotes() {
        let query = "$.foo['bar']";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::SingleQuoteString {
                        value: "bar".to_string().into_boxed_str()
                    },
                    7,
                    10
                ),
                Token::new(TokenType::RBracket, 11, 12),
                Token::new(TokenType::Eoq, 12, 12),
            ]
        )
    }

    #[test]
    fn multiple_selectors() {
        let query = "$.foo['bar', 123, *]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::SingleQuoteString {
                        value: "bar".to_string().into_boxed_str()
                    },
                    7,
                    10
                ),
                Token::new(TokenType::Comma, 11, 12),
                Token::new(
                    TokenType::Index {
                        value: "123".to_string().into_boxed_str()
                    },
                    13,
                    16
                ),
                Token::new(TokenType::Comma, 16, 17),
                Token::new(TokenType::Wild, 18, 19),
                Token::new(TokenType::RBracket, 19, 20),
                Token::new(TokenType::Eoq, 20, 20),
            ]
        )
    }

    #[test]
    fn slice() {
        let query = "$.foo[1:3]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(
                    TokenType::Index {
                        value: "1".to_string().into_boxed_str()
                    },
                    6,
                    7
                ),
                Token::new(TokenType::Colon, 7, 8),
                Token::new(
                    TokenType::Index {
                        value: "3".to_string().into_boxed_str()
                    },
                    8,
                    9
                ),
                Token::new(TokenType::RBracket, 9, 10),
                Token::new(TokenType::Eoq, 10, 10),
            ]
        )
    }

    #[test]
    fn filter() {
        let query = "$.foo[?@.bar]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(TokenType::Filter, 6, 7),
                Token::new(TokenType::Current, 7, 8),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    9,
                    12
                ),
                Token::new(TokenType::RBracket, 12, 13),
                Token::new(TokenType::Eoq, 13, 13),
            ]
        )
    }

    #[test]
    fn filter_single_quoted_string() {
        let query = "$.foo[?@.bar == 'baz']";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(TokenType::Filter, 6, 7),
                Token::new(TokenType::Current, 7, 8),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    9,
                    12
                ),
                Token::new(TokenType::Eq, 13, 15),
                Token::new(
                    TokenType::SingleQuoteString {
                        value: "baz".to_string().into_boxed_str()
                    },
                    17,
                    20
                ),
                Token::new(TokenType::RBracket, 21, 22),
                Token::new(TokenType::Eoq, 22, 22),
            ]
        )
    }

    #[test]
    fn filter_double_quoted_string() {
        let query = "$.foo[?@.bar == \"baz\"]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(TokenType::Filter, 6, 7),
                Token::new(TokenType::Current, 7, 8),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    9,
                    12
                ),
                Token::new(TokenType::Eq, 13, 15),
                Token::new(
                    TokenType::DoubleQuoteString {
                        value: "baz".to_string().into_boxed_str()
                    },
                    17,
                    20
                ),
                Token::new(TokenType::RBracket, 21, 22),
                Token::new(TokenType::Eoq, 22, 22),
            ]
        )
    }

    #[test]
    fn filter_parenthesized_expression() {
        let query = "$.foo[?(@.bar)]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(TokenType::Filter, 6, 7),
                Token::new(TokenType::LParen, 7, 8),
                Token::new(TokenType::Current, 8, 9),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    10,
                    13
                ),
                Token::new(TokenType::RParen, 13, 14),
                Token::new(TokenType::RBracket, 14, 15),
                Token::new(TokenType::Eoq, 15, 15),
            ]
        )
    }

    #[test]
    fn two_filters() {
        let query = "$.foo[?@.bar, ?@.baz]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    2,
                    5
                ),
                Token::new(TokenType::LBracket, 5, 6),
                Token::new(TokenType::Filter, 6, 7),
                Token::new(TokenType::Current, 7, 8),
                Token::new(
                    TokenType::Name {
                        value: "bar".to_string().into_boxed_str()
                    },
                    9,
                    12
                ),
                Token::new(TokenType::Comma, 12, 13),
                Token::new(TokenType::Filter, 14, 15),
                Token::new(TokenType::Current, 15, 16),
                Token::new(
                    TokenType::Name {
                        value: "baz".to_string().into_boxed_str()
                    },
                    17,
                    20
                ),
                Token::new(TokenType::RBracket, 20, 21),
                Token::new(TokenType::Eoq, 21, 21),
            ]
        )
    }

    #[test]
    fn filter_function() {
        let query = "$[?count(@.foo)>2]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(
                    TokenType::Function {
                        name: "count".to_string().into_boxed_str()
                    },
                    3,
                    8,
                ),
                Token::new(TokenType::Current, 9, 10),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    11,
                    14
                ),
                Token::new(TokenType::RParen, 14, 15),
                Token::new(TokenType::Gt, 15, 16),
                Token::new(
                    TokenType::Int {
                        value: "2".to_string().into_boxed_str()
                    },
                    16,
                    17
                ),
                Token::new(TokenType::RBracket, 17, 18),
                Token::new(TokenType::Eoq, 18, 18),
            ]
        )
    }

    #[test]
    fn filter_function_with_two_args() {
        let query = "$[?count(@.foo, 1)>2]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(
                    TokenType::Function {
                        name: "count".to_string().into_boxed_str()
                    },
                    3,
                    8
                ),
                Token::new(TokenType::Current, 9, 10),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    11,
                    14
                ),
                Token::new(TokenType::Comma, 14, 15),
                Token::new(
                    TokenType::Int {
                        value: "1".to_string().into_boxed_str()
                    },
                    16,
                    17
                ),
                Token::new(TokenType::RParen, 17, 18),
                Token::new(TokenType::Gt, 18, 19),
                Token::new(
                    TokenType::Int {
                        value: "2".to_string().into_boxed_str()
                    },
                    19,
                    20
                ),
                Token::new(TokenType::RBracket, 20, 21),
                Token::new(TokenType::Eoq, 21, 21),
            ]
        )
    }

    #[test]
    fn filter_parenthesized_function() {
        let query = "$[?(count(@.foo)>2)]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::LParen, 3, 4),
                Token::new(
                    TokenType::Function {
                        name: "count".to_string().into_boxed_str()
                    },
                    4,
                    9
                ),
                Token::new(TokenType::Current, 10, 11),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    12,
                    15
                ),
                Token::new(TokenType::RParen, 15, 16),
                Token::new(TokenType::Gt, 16, 17),
                Token::new(
                    TokenType::Int {
                        value: "2".to_string().into_boxed_str()
                    },
                    17,
                    18
                ),
                Token::new(TokenType::RParen, 18, 19),
                Token::new(TokenType::RBracket, 19, 20),
                Token::new(TokenType::Eoq, 20, 20),
            ]
        )
    }

    #[test]
    fn filter_parenthesized_function_argument() {
        let query = "$[?(count((@.foo),1)>2)]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::LParen, 3, 4),
                Token::new(
                    TokenType::Function {
                        name: "count".to_string().into_boxed_str()
                    },
                    4,
                    9
                ),
                Token::new(TokenType::LParen, 10, 11),
                Token::new(TokenType::Current, 11, 12),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    13,
                    16
                ),
                Token::new(TokenType::RParen, 16, 17),
                Token::new(TokenType::Comma, 17, 18),
                Token::new(
                    TokenType::Int {
                        value: "1".to_string().into_boxed_str()
                    },
                    18,
                    19
                ),
                Token::new(TokenType::RParen, 19, 20),
                Token::new(TokenType::Gt, 20, 21),
                Token::new(
                    TokenType::Int {
                        value: "2".to_string().into_boxed_str()
                    },
                    21,
                    22
                ),
                Token::new(TokenType::RParen, 22, 23),
                Token::new(TokenType::RBracket, 23, 24),
                Token::new(TokenType::Eoq, 24, 24),
            ]
        )
    }

    #[test]
    fn filter_nested() {
        let query = "$[?@[?@>1]]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::Current, 3, 4),
                Token::new(TokenType::LBracket, 4, 5),
                Token::new(TokenType::Filter, 5, 6),
                Token::new(TokenType::Current, 6, 7),
                Token::new(TokenType::Gt, 7, 8),
                Token::new(
                    TokenType::Int {
                        value: "1".to_string().into_boxed_str()
                    },
                    8,
                    9
                ),
                Token::new(TokenType::RBracket, 9, 10),
                Token::new(TokenType::RBracket, 10, 11),
                Token::new(TokenType::Eoq, 11, 11),
            ]
        )
    }

    #[test]
    fn filter_nested_brackets() {
        let query = "$[?@[?@[1]>1]]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::Current, 3, 4),
                Token::new(TokenType::LBracket, 4, 5),
                Token::new(TokenType::Filter, 5, 6),
                Token::new(TokenType::Current, 6, 7),
                Token::new(TokenType::LBracket, 7, 8),
                Token::new(
                    TokenType::Index {
                        value: "1".to_string().into_boxed_str()
                    },
                    8,
                    9
                ),
                Token::new(TokenType::RBracket, 9, 10),
                Token::new(TokenType::Gt, 10, 11),
                Token::new(
                    TokenType::Int {
                        value: "1".to_string().into_boxed_str()
                    },
                    11,
                    12
                ),
                Token::new(TokenType::RBracket, 12, 13),
                Token::new(TokenType::RBracket, 13, 14),
                Token::new(TokenType::Eoq, 14, 14),
            ]
        )
    }

    #[test]
    fn function() {
        let query = "$[?foo()]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(
                    TokenType::Function {
                        name: "foo".to_string().into_boxed_str()
                    },
                    3,
                    6
                ),
                Token::new(TokenType::RParen, 7, 8),
                Token::new(TokenType::RBracket, 8, 9),
                Token::new(TokenType::Eoq, 9, 9),
            ]
        )
    }

    #[test]
    fn function_int_literal() {
        let query = "$[?foo(42)]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(
                    TokenType::Function {
                        name: "foo".to_string().into_boxed_str()
                    },
                    3,
                    6
                ),
                Token::new(
                    TokenType::Int {
                        value: "42".to_string().into_boxed_str()
                    },
                    7,
                    9
                ),
                Token::new(TokenType::RParen, 9, 10),
                Token::new(TokenType::RBracket, 10, 11),
                Token::new(TokenType::Eoq, 11, 11),
            ]
        )
    }

    #[test]
    fn function_two_int_args() {
        let query = "$[?foo(42, -7)]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(
                    TokenType::Function {
                        name: "foo".to_string().into_boxed_str()
                    },
                    3,
                    6
                ),
                Token::new(
                    TokenType::Int {
                        value: "42".to_string().into_boxed_str()
                    },
                    7,
                    9
                ),
                Token::new(TokenType::Comma, 9, 10),
                Token::new(
                    TokenType::Int {
                        value: "-7".to_string().into_boxed_str()
                    },
                    11,
                    13
                ),
                Token::new(TokenType::RParen, 13, 14),
                Token::new(TokenType::RBracket, 14, 15),
                Token::new(TokenType::Eoq, 15, 15),
            ]
        )
    }

    #[test]
    fn boolean_literals() {
        let query = "$[?true==false]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::True, 3, 7),
                Token::new(TokenType::Eq, 7, 9),
                Token::new(TokenType::False, 9, 14),
                Token::new(TokenType::RBracket, 14, 15),
                Token::new(TokenType::Eoq, 15, 15)
            ]
        )
    }

    #[test]
    fn logical_and() {
        let query = "$[?true && false]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::True, 3, 7),
                Token::new(TokenType::And, 8, 10),
                Token::new(TokenType::False, 11, 16),
                Token::new(TokenType::RBracket, 16, 17),
                Token::new(TokenType::Eoq, 17, 17),
            ]
        )
    }

    #[test]
    fn float() {
        let query = "$[?@.foo > 42.7]";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(TokenType::LBracket, 1, 2),
                Token::new(TokenType::Filter, 2, 3),
                Token::new(TokenType::Current, 3, 4),
                Token::new(
                    TokenType::Name {
                        value: "foo".to_string().into_boxed_str()
                    },
                    5,
                    8
                ),
                Token::new(TokenType::Gt, 9, 10),
                Token::new(
                    TokenType::Float {
                        value: "42.7".to_string().into_boxed_str()
                    },
                    11,
                    15
                ),
                Token::new(TokenType::RBracket, 15, 16),
                Token::new(TokenType::Eoq, 16, 16),
            ]
        )
    }

    #[test]
    fn unexpected_shorthand() {
        let query = "$.5";
        let tokens = tokenize(query);
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenType::Root, 0, 1),
                Token::new(
                    TokenType::Error {
                        msg: "unexpected shorthand selector '5'"
                            .to_string()
                            .into_boxed_str()
                    },
                    2,
                    3
                ),
            ]
        )
    }
}
