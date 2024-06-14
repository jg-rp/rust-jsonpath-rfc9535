use std::{
    cmp,
    fmt::{self, Write},
};

use serde_json::Value;

use crate::filter::FilterExpression;

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

pub fn slice<'a>(
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

pub fn norm_index(index: i64, length: usize) -> usize {
    if index < 0 && length >= index.abs() as usize {
        (length as i64 + index) as usize
    } else {
        index as usize
    }
}
