use lazy_static::lazy_static;
use serde_json::Value;

use crate::{
    conslist::ConsList,
    env::Environment,
    errors::JSONPathError,
    node::{Node, NodeList},
    segment::Segment,
    selector::Selector,
    JSONPathParser,
};

lazy_static! {
    static ref PARSER: JSONPathParser = JSONPathParser::new();
}

#[derive(Debug)]
pub struct Query {
    pub segments: Vec<Segment>,
}

impl Query {
    pub fn new(segments: Vec<Segment>) -> Self {
        Query { segments }
    }

    pub fn standard(expr: &str) -> Result<Self, JSONPathError> {
        PARSER.parse(expr)
    }

    pub fn find<'v>(&self, value: &'v Value, env: &'static Environment) -> NodeList<'v> {
        let root_node = Node {
            value,
            location: ConsList::new(),
        };

        self.segments
            .iter()
            .fold(vec![root_node], |nodes, segment| {
                segment.resolve(nodes, env, value)
            })
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn is_singular(&self) -> bool {
        self.segments.iter().all(|segment| {
            if let Segment::Child { selectors, .. } = segment {
                return selectors.len() == 1
                    && selectors.first().is_some_and(|selector| {
                        matches!(selector, Selector::Name { .. } | Selector::Index { .. })
                    });
            }
            false
        })
    }
}
