use std::{
    cmp,
    fmt::{self, Write},
};

use serde_json::Value;

use crate::{
    env::Environment,
    filter::{is_truthy_ref, FilterExpression},
    node::{Location, Node, NodeList},
};

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
    Wild,
    Filter {
        expression: Box<FilterExpression>,
    },
}

impl Selector {
    pub fn resolve<'v>(
        &self,
        env: &'static Environment,
        value: &'v Value,
        root: &'v Value,
        location: &Location,
    ) -> NodeList<'v> {
        match self {
            Selector::Name { name } => value
                .as_object()
                .and_then(|m| m.get_key_value(name))
                .and_then(|(k, v)| Some(Node::new_object_member(v, location, k.to_owned())))
                .into_iter()
                .collect(),
            Selector::Index { index } => value
                .as_array()
                .and_then(|array| Some((norm_index(*index, array.len())?, array)))
                .and_then(|(i, array)| Some((i, array.get(i)?)))
                .and_then(|(i, v)| Some(Node::new_array_element(v, location, i)))
                .into_iter()
                .collect(),
            Selector::Slice { start, stop, step } => {
                if let Some(array) = value.as_array() {
                    slice(array, *start, *stop, *step)
                        .into_iter()
                        .map(|(i, v)| Node::new_array_element(v, location, i as usize))
                        .collect()
                } else {
                    Vec::new()
                }
            }
            Selector::Wild {} => match value {
                Value::Array(arr) => arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Node::new_array_element(v, location, i))
                    .collect(),
                Value::Object(obj) => obj
                    .iter()
                    .map(|(k, v)| Node::new_object_member(v, location, k.to_owned()))
                    .collect(),
                _ => Vec::new(),
            },
            Selector::Filter { expression } => match value {
                Value::Array(arr) => arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, v, expression.evaluate(env, root, v)))
                    .filter(|(_, _, r)| is_truthy_ref(r))
                    .map(|(i, v, _)| Node::new_array_element(v, location, i))
                    .collect(),
                Value::Object(obj) => obj
                    .iter()
                    .map(|(k, v)| (k, v, expression.evaluate(env, root, v)))
                    .filter(|(_, _, r)| is_truthy_ref(r))
                    .map(|(k, v, _)| Node::new_object_member(v, location, k.to_owned()))
                    .collect(),
                _ => Vec::new(),
            },
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

fn norm_index(index: i64, length: usize) -> Option<usize> {
    if index < 0 {
        index
            .checked_abs()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| length.checked_sub(i))
    } else {
        usize::try_from(index).ok()
    }
}

fn slice(
    array: &[Value],
    start: Option<i64>,
    stop: Option<i64>,
    step: Option<i64>,
) -> Vec<(i64, &Value)> {
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
