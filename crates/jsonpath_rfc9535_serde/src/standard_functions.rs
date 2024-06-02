use std::{num::NonZeroUsize, sync::Mutex};

use lru::LruCache;
use regex::Regex;
use serde_json;

use crate::{
    ast::FilterExpressionResult,
    function::{ExpressionType, FunctionExtension, FunctionSignature},
};

pub struct Count;

impl Count {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Count {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionExtension for Count {
    fn call(&self, args: Vec<FilterExpressionResult>) -> FilterExpressionResult {
        match args.first().unwrap() {
            FilterExpressionResult::Nodes(nodes) => {
                FilterExpressionResult::Value(serde_json::Value::Number(nodes.len().into()))
            }
            _ => unreachable!(),
        }
    }

    fn sig(&self) -> FunctionSignature {
        FunctionSignature {
            param_types: vec![ExpressionType::Nodes],
            return_type: ExpressionType::Value,
        }
    }
}

pub struct Length;

impl Length {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Length {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionExtension for Length {
    fn call(&self, args: Vec<FilterExpressionResult>) -> FilterExpressionResult {
        match args.first().unwrap() {
            FilterExpressionResult::Value(value) => match value {
                serde_json::Value::String(s) => FilterExpressionResult::Value(
                    serde_json::Value::Number(s.chars().count().into()),
                ),
                serde_json::Value::Array(a) => {
                    FilterExpressionResult::Value(serde_json::Value::Number(a.len().into()))
                }
                serde_json::Value::Object(o) => {
                    FilterExpressionResult::Value(serde_json::Value::Number(o.len().into()))
                }
                _ => FilterExpressionResult::Nothing,
            },
            FilterExpressionResult::Nothing => FilterExpressionResult::Nothing,
            _ => unreachable!(),
        }
    }

    fn sig(&self) -> FunctionSignature {
        FunctionSignature {
            param_types: vec![ExpressionType::Value],
            return_type: ExpressionType::Value,
        }
    }
}

pub struct Match {
    cache: Mutex<LruCache<String, Regex>>,
}

impl Match {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
        }
    }
}

impl Default for Match {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionExtension for Match {
    fn call(&self, args: Vec<FilterExpressionResult>) -> FilterExpressionResult {
        match (args.first().unwrap(), args.get(1).unwrap()) {
            (FilterExpressionResult::Value(v), FilterExpressionResult::Value(u)) => {
                if !v.is_string() || !u.is_string() {
                    return FilterExpressionResult::Value(serde_json::Value::Bool(false));
                }

                let s = v.as_str().unwrap();
                let p = u.as_str().unwrap();

                // TODO: fail early if p is known to be invalid
                let mut cache = self.cache.lock().unwrap();

                match cache.get(p) {
                    Some(re) => {
                        FilterExpressionResult::Value(serde_json::Value::Bool(re.is_match(s)))
                    }
                    None => {
                        if !iregexp::check(p) {
                            return FilterExpressionResult::Value(serde_json::Value::Bool(false));
                        }

                        if let Ok(re) = Regex::new(&full_match(&p)) {
                            let rv = re.is_match(s);
                            cache.push(p.to_owned(), re);
                            FilterExpressionResult::Value(serde_json::Value::Bool(rv))
                        } else {
                            FilterExpressionResult::Value(serde_json::Value::Bool(false))
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn sig(&self) -> FunctionSignature {
        FunctionSignature {
            param_types: vec![ExpressionType::Value, ExpressionType::Value],
            return_type: ExpressionType::Logical,
        }
    }
}

pub struct Search {
    cache: Mutex<LruCache<String, Regex>>,
}

impl Search {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
        }
    }
}

impl Default for Search {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionExtension for Search {
    fn call(&self, args: Vec<FilterExpressionResult>) -> FilterExpressionResult {
        match (args.first().unwrap(), args.get(1).unwrap()) {
            (FilterExpressionResult::Value(v), FilterExpressionResult::Value(u)) => {
                if !v.is_string() || !u.is_string() {
                    return FilterExpressionResult::Value(serde_json::Value::Bool(false));
                }

                let s = v.as_str().unwrap();
                let p = u.as_str().unwrap();

                // TODO: fail early if p is known to be invalid
                let mut cache = self.cache.lock().unwrap();

                match cache.get(p) {
                    Some(re) => {
                        FilterExpressionResult::Value(serde_json::Value::Bool(re.is_match(s)))
                    }
                    None => {
                        if !iregexp::check(p) {
                            return FilterExpressionResult::Value(serde_json::Value::Bool(false));
                        }

                        if let Ok(re) = Regex::new(&map_regex(&p)) {
                            let rv = re.is_match(s);
                            cache.push(p.to_owned(), re);
                            FilterExpressionResult::Value(serde_json::Value::Bool(rv))
                        } else {
                            println!("re compilation failed {:#?}", Regex::new(&map_regex(&p)));
                            FilterExpressionResult::Value(serde_json::Value::Bool(false))
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn sig(&self) -> FunctionSignature {
        FunctionSignature {
            param_types: vec![ExpressionType::Value, ExpressionType::Value],
            return_type: ExpressionType::Logical,
        }
    }
}

pub struct Value;

impl Value {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionExtension for Value {
    fn call(&self, args: Vec<FilterExpressionResult>) -> FilterExpressionResult {
        match args.first().unwrap() {
            FilterExpressionResult::Nodes(nodes) => {
                if nodes.len() == 1 {
                    FilterExpressionResult::Value(nodes.first().unwrap().value.clone())
                } else {
                    FilterExpressionResult::Nothing
                }
            }
            _ => unreachable!(),
        }
    }

    fn sig(&self) -> FunctionSignature {
        FunctionSignature {
            param_types: vec![ExpressionType::Nodes],
            return_type: ExpressionType::Value,
        }
    }
}

/// Map re pattern to i-regexp pattern.
fn map_regex(pattern: &str) -> String {
    // let mut escaped = false;
    // let mut char_class = false;
    // let mut parts: Vec<String> = Vec::new();

    // for c in pattern.chars() {
    //     if escaped {
    //         parts.push(String::from(c));
    //         escaped = false;
    //         continue;
    //     }

    //     match c {
    //         '.' => {
    //             if !char_class {
    //                 parts.push(String::from("(?:(?![\r\n])\\P{Cs}|\\p{Cs}\\p{Cs})"));
    //             } else {
    //                 parts.push(String::from(c));
    //             }
    //         }
    //         '\\' => {
    //             escaped = true;
    //             parts.push(String::from(c));
    //         }
    //         '[' => {
    //             char_class = true;
    //             parts.push(String::from(c));
    //         }
    //         ']' => {
    //             char_class = false;
    //             parts.push(String::from(c));
    //         }
    //         _ => parts.push(String::from(c)),
    //     }
    // }

    // parts.join("");
    pattern.to_owned()
}

fn full_match(pattern: &str) -> String {
    if !pattern.starts_with('^') && !pattern.ends_with('$') {
        map_regex(&format!("^(?:{})$", pattern))
    } else {
        map_regex(pattern)
    }
}
