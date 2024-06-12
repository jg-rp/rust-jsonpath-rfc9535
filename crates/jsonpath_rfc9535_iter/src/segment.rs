use std::{fmt, iter, rc::Rc};

use serde_json::Value;

use crate::{
    node::{Node, NodeIter},
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

// pub fn visit<'v>(value: &'v Value) -> NodeList<'v> {
//     let mut values: NodeList = vec![value];

//     match value {
//         Value::Object(obj) => {
//             obj.iter().for_each(|(_k, v)| values.extend(visit(v)));
//         }
//         Value::Array(arr) => arr
//             .iter()
//             .enumerate()
//             .for_each(|(_i, e)| values.extend(visit(e))),
//         _ => (),
//     }

//     values
// }

pub fn visit_iter<'v>(node: Rc<Node<'v>>) -> NodeIter<'v> {
    Box::new(iter::once(node.clone()).chain(descendants(node.clone())))
}

pub fn descendants<'v>(node: Rc<Node<'v>>) -> NodeIter<'v> {
    match node.value {
        Value::Object(obj) => Box::new(obj.iter().flat_map(move |(k, v)| {
            let child = node.new_child_member(v, k);
            iter::once(child.clone()).chain(descendants(child))
        })),

        Value::Array(arr) => Box::new(arr.iter().enumerate().flat_map(move |(i, e)| {
            let child = node.new_child_element(e, i);
            iter::once(child.clone()).chain(descendants(child))
        })),
        _ => Box::new(iter::empty()),
    }
}
