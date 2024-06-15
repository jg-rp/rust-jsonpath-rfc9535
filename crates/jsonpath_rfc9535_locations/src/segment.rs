use std::fmt;

use serde_json::Value;

use crate::{
    env::Environment,
    node::{Location, NodeList},
    selector::Selector,
};

#[derive(Debug)]
pub enum Segment {
    Child { selectors: Vec<Selector> },
    Recursive { selectors: Vec<Selector> },
    Eoi,
}

impl Segment {
    pub fn resolve<'v>(
        &self,
        nodes: NodeList<'v>,
        env: &'static Environment,
        root: &'v Value,
    ) -> NodeList<'v> {
        match self {
            Segment::Child { selectors } => nodes
                .into_iter()
                .flat_map(|node| {
                    selectors
                        .iter()
                        .map(move |s| s.resolve(env, node.value, root, &node.location))
                })
                .flatten()
                .collect(),
            Segment::Recursive { selectors } => nodes
                .into_iter()
                .flat_map(move |node| self.visit(env, node.value, selectors, root, &node.location))
                .collect(),
            Segment::Eoi {} => nodes,
        }
    }

    fn visit<'v>(
        &self,
        env: &'static Environment,
        value: &'v Value,
        selectors: &Vec<Selector>,
        root: &'v Value,
        location: &Location,
    ) -> NodeList<'v> {
        let mut nodes: NodeList = selectors
            .iter()
            .flat_map(|s| s.resolve(env, value, root, &location))
            .collect();

        nodes.append(&mut self.descend(env, value, selectors, root, &location));
        nodes
    }

    fn descend<'v>(
        &self,
        env: &'static Environment,
        value: &'v Value,
        selectors: &Vec<Selector>,
        root: &'v Value,
        location: &Location,
    ) -> NodeList<'v> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .enumerate()
                .flat_map(|(i, v)| {
                    location.append(crate::node::PathElement::Index(i));
                    self.visit(env, v, selectors, root, &location)
                })
                .collect(),
            Value::Object(obj) => obj
                .iter()
                .flat_map(|(k, v)| {
                    location.append(crate::node::PathElement::Name(k.to_owned()));
                    self.visit(env, v, selectors, root, &location)
                })
                .collect(),
            _ => vec![],
        }
    }
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
