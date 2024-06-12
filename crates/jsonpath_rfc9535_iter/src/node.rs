use std::rc::Rc;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Node<'a> {
    pub value: &'a Value,
    pub location: String, // TODO: cow
}

impl<'a> Node<'a> {
    pub fn new_child_member(&self, value: &'a Value, loc: &str) -> Rc<Self> {
        Rc::new(Node {
            value,
            location: format!("{}['{}']", self.location, loc),
        })
    }

    pub fn new_child_element(&self, value: &'a Value, loc: usize) -> Rc<Self> {
        Rc::new(Node {
            value,
            location: format!("{}[{}]", self.location, loc),
        })
    }
}

pub type NodeList<'v> = Vec<Rc<Node<'v>>>;
pub type NodeIter<'v> = Box<dyn Iterator<Item = Rc<Node<'v>>> + 'v>;

// TODO: cow Rc<Node> / Node