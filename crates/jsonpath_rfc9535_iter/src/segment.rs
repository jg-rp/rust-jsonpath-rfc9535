use std::{fmt, iter};

use serde_json::Value;

use crate::{
    node::{NodeIter, NodeList},
    selector::Selector,
};

#[derive(Debug, Clone)]
pub enum Segment {
    Child { selectors: Vec<Selector> },
    Recursive { selectors: Vec<Selector> },
    Eoi,
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
            Segment::Eoi => Ok(()),
        }
    }
}

pub fn visit<'v>(value: &'v Value) -> NodeList<'v> {
    let mut values: NodeList = vec![value];

    match value {
        Value::Object(obj) => {
            obj.iter().for_each(|(_k, v)| values.extend(visit(v)));
        }
        Value::Array(arr) => arr
            .iter()
            .enumerate()
            .for_each(|(_i, e)| values.extend(visit(e))),
        _ => (),
    }

    values
}

pub fn visit_iter<'v>(value: &'v Value) -> NodeIter<'v> {
    Box::new(iter::once(value).chain(descendants(value)))
}

pub fn descendants<'v>(value: &'v Value) -> NodeIter<'v> {
    match value {
        Value::Object(obj) => Box::new(
            obj.iter()
                .flat_map(|(_k, v)| iter::once(v).chain(descendants(v))),
        ),
        Value::Array(arr) => Box::new(arr.iter().flat_map(|e| iter::once(e).chain(descendants(e)))),
        _ => Box::new(iter::empty()),
    }
}
