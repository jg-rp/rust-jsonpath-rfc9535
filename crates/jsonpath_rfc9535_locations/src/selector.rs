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
                .map(|(k, v)| Node::new_object_member(v, location, k.to_owned()))
                .into_iter()
                .collect(),
            Selector::Index { index } => value
                .as_array()
                .and_then(|array| Some((norm_index(*index, array.len())?, array)))
                .and_then(|(i, array)| Some((i, array.get(i)?)))
                .map(|(i, v)| Node::new_array_element(v, location, i))
                .into_iter()
                .collect(),
            Selector::Slice { start, stop, step } => value
                .as_array()
                .and_then(|array| slice(array, location, *start, *stop, *step))
                .unwrap_or_default(),
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

fn slice<'v>(
    array: &'v [Value],
    location: &Location,
    start: Option<i64>,
    stop: Option<i64>,
    step: Option<i64>,
) -> Option<NodeList<'v>> {
    let len = array.len() as i128;

    if len == 0 {
        return None;
    }

    let step = step.unwrap_or(1);

    if step == 0 {
        return None;
    }

    let n_start = match start {
        Some(i) => {
            if i < 0 {
                cmp::max(len + i as i128, 0)
            } else {
                cmp::min(i as i128, len - 1)
            }
        }
        None => {
            if step < 0 {
                len - 1
            } else {
                0
            }
        }
    };

    let n_stop = match stop {
        Some(i) => {
            if i < 0 {
                cmp::max(len + i as i128, -1)
            } else {
                cmp::min(i as i128, len)
            }
        }
        None => {
            if step < 0 {
                -1
            } else {
                len
            }
        }
    };

    let mut slice: NodeList = Vec::new();

    if step > 0 {
        let step = usize::try_from(step).ok()?;
        for i in (n_start..n_stop).step_by(step) {
            let index = usize::try_from(i).ok()?;
            slice.push(Node::new_array_element(
                array.get(index).unwrap(),
                location,
                index,
            ));
        }
    } else {
        let step = step as i128;
        let mut i = n_start;
        while i > n_stop {
            let index = usize::try_from(i).ok()?;
            slice.push(Node::new_array_element(
                array.get(index).unwrap(),
                location,
                index,
            ));
            i += step;
        }
    }

    Some(slice)
}
