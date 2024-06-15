use std::{collections::VecDeque, iter};

use crate::conslist::ConsList;
use serde_json::Value;

pub type Location = ConsList<PathElement>;
pub type NodeList<'v> = Vec<Node<'v>>;

#[derive(Debug)]
pub struct Node<'v> {
    pub value: &'v Value,
    pub location: Location,
}

/// An array element index or object member name in a Node's location.
#[derive(Debug, Clone)]
pub enum PathElement {
    Index(usize),
    Name(String),
}

impl<'v> Node<'v> {
    pub fn new_array_element(value: &'v Value, location: Location, index: usize) -> Self {
        location.append(PathElement::Index(index));
        Node { value, location }
    }

    pub fn new_object_member(value: &'v Value, location: Location, name: String) -> Self {
        location.append(PathElement::Name(name));
        Node { value, location }
    }

    /// The location of this node's value in the query argument as a normalized path.
    pub fn path(&self) -> String {
        iter::once(String::from("$"))
            .chain(
                VecDeque::from_iter(self.location.iter().map(|e| match e {
                    PathElement::Index(i) => format!("[{}]", i),
                    PathElement::Name(s) => format!("['{}']", s),
                }))
                .into_iter()
                .rev(),
            )
            .collect::<Vec<String>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_path_names() {
        let location = ConsList::from_iter(vec![
            PathElement::Name(String::from("a")),
            PathElement::Name(String::from("b")),
            PathElement::Name(String::from("c")),
        ]);

        let value = Value::Bool(true);
        let node = Node {
            value: &value,
            location,
        };

        assert_eq!(node.path(), "$['a']['b']['c']");
    }

    #[test]
    fn normalized_path_indices() {
        let location = ConsList::from_iter(vec![
            PathElement::Index(1),
            PathElement::Index(2),
            PathElement::Index(3),
        ]);

        let value = Value::Bool(true);
        let node = Node {
            value: &value,
            location,
        };

        assert_eq!(node.path(), "$[1][2][3]");
    }

    #[test]
    fn normalized_path_mixed() {
        let location = ConsList::from_iter(vec![
            PathElement::Name(String::from("a")),
            PathElement::Index(2),
            PathElement::Name(String::from("c")),
        ]);

        let value = Value::Bool(true);
        let node = Node {
            value: &value,
            location,
        };

        assert_eq!(node.path(), "$['a'][2]['c']");
    }

    #[test]
    fn normalized_path_root() {
        let location = ConsList::new();
        let value = Value::Bool(true);
        let node = Node {
            value: &value,
            location,
        };

        assert_eq!(node.path(), "$");
    }
}
