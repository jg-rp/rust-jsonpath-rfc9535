use std::{
    iter::{self, Enumerate},
    rc::Rc,
    slice::Iter,
    vec::IntoIter,
};

use serde_json::{Map, Value};

use crate::{
    env::Environment,
    filter::{is_truthy, FilterExpression},
    node::{Node, NodeIter},
    segment::{visit_iter, Segment},
    selector::{norm_index, slice, Selector},
    Query,
};

pub struct QueryIter<'v> {
    it: NodeIter<'v>, // Just SegmentIter
}

impl<'v> Iterator for QueryIter<'v> {
    type Item = Rc<Node<'v>>;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

impl<'v> QueryIter<'v> {
    pub fn new(env: &'static Environment, root: &'v Value, query: Query) -> Self {
        let init = SegmentIter {
            selectors: vec![].into_iter(),
            it: Box::new(iter::once(Rc::new(Node {
                value: root,
                location: String::from("$"),
            }))),
        };

        let it = query
            .segments
            .into_iter()
            .filter(|s| !matches!(s, Segment::Eoi {}))
            .fold(init, |values, segment| {
                SegmentIter::new(env, root, segment, Box::new(values))
            });

        Self { it: Box::new(it) }
    }
}

pub struct SegmentIter<'v> {
    selectors: IntoIter<SelectorIter<'v>>,
    it: NodeIter<'v>,
}

impl<'v> Iterator for SegmentIter<'v> {
    type Item = Rc<Node<'v>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some(v) => Some(v),
            None => match self.selectors.next() {
                Some(it) => {
                    self.it = Box::new(it);
                    self.next()
                }
                None => None,
            },
        }
    }
}

impl<'v> SegmentIter<'v> {
    pub fn new(
        env: &'static Environment,
        root: &'v Value,
        segment: Segment,
        nodes: NodeIter<'v>,
    ) -> Self {
        let mut its: Vec<SelectorIter<'v>> = Vec::new();
        match segment {
            Segment::Child { ref selectors } => {
                for node in nodes {
                    for selector in selectors {
                        its.push(SelectorIter::new(env, root, selector.clone(), node.clone()))
                    }
                }
            }
            Segment::Recursive { ref selectors } => {
                for node in nodes {
                    for _node in visit_iter(node) {
                        for selector in selectors {
                            its.push(SelectorIter::new(
                                env,
                                root,
                                selector.clone(),
                                _node.clone(),
                            ))
                        }
                    }
                }
            }
            Segment::Eoi {} => unreachable!(),
        };

        let mut its = its.into_iter();
        let it: NodeIter<'v> = match its.next() {
            Some(sel) => Box::new(sel),
            None => Box::new(iter::empty()),
        };

        Self { selectors: its, it }
    }
}

pub struct SelectorIter<'v> {
    it: NodeIter<'v>,
}

impl<'v> Iterator for SelectorIter<'v> {
    type Item = Rc<Node<'v>>;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

impl<'v> SelectorIter<'v> {
    fn new(
        env: &'static Environment,
        root: &'v Value,
        selector: Selector,
        node: Rc<Node<'v>>,
    ) -> Self {
        let it: NodeIter<'v> = match selector {
            Selector::Name { ref name } => {
                if let Some(v) = node.value.get(name) {
                    Box::new(iter::once(node.new_child_member(v, name)))
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Index { index } => {
                if let Some(array) = node.value.as_array() {
                    let norm = norm_index(index, array.len());
                    if let Some(v) = array.get(norm) {
                        Box::new(iter::once(node.new_child_element(v, norm)))
                    } else {
                        Box::new(iter::empty())
                    }
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Slice { start, stop, step } => {
                if let Some(array) = node.value.as_array() {
                    // TODO: lazy slice
                    Box::new(
                        slice(array, start, stop, step)
                            .into_iter()
                            .map(move |(i, v)| node.new_child_element(v, i as usize)),
                    )
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Wild {} => match node.value {
                Value::Array(arr) => Box::new(
                    arr.iter()
                        .enumerate()
                        .map(move |(i, v)| node.new_child_element(v, i)),
                ),
                Value::Object(obj) => {
                    Box::new(obj.iter().map(move |(k, v)| node.new_child_member(v, k)))
                }
                _ => Box::new(iter::empty()),
            },
            Selector::Filter { expression } => match node.value {
                Value::Array(arr) => {
                    Box::new(ArrayFilterIter::new(env, root, *expression, &arr, node))
                }
                Value::Object(obj) => {
                    Box::new(ObjectFilterIter::new(env, root, *expression, &obj, node))
                }
                _ => Box::new(iter::empty()),
            },
        };

        Self { it }
    }
}

pub struct ArrayFilterIter<'v> {
    env: &'static Environment,
    root: &'v Value,
    expr: FilterExpression,
    it: Enumerate<Iter<'v, Value>>,
    parent: Rc<Node<'v>>,
}

impl<'v> Iterator for ArrayFilterIter<'v> {
    type Item = Rc<Node<'v>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some((index, value)) => {
                if is_truthy(self.expr.evaluate(self.env, self.root, value)) {
                    Some(self.parent.new_child_element(value, index))
                } else {
                    self.next()
                }
            }
            None => None,
        }
    }
}

impl<'v> ArrayFilterIter<'v> {
    fn new(
        env: &'static Environment,
        root: &'v Value,
        expr: FilterExpression,
        arr: &'v Vec<Value>,
        node: Rc<Node<'v>>,
    ) -> Self {
        Self {
            env,
            root,
            expr,
            it: arr.iter().enumerate(),
            parent: node,
        }
    }
}

pub struct ObjectFilterIter<'v> {
    env: &'static Environment,
    root: &'v Value,
    expr: FilterExpression,
    it: serde_json::map::Iter<'v>,
    parent: Rc<Node<'v>>,
}

impl<'v> Iterator for ObjectFilterIter<'v> {
    type Item = Rc<Node<'v>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some((k, v)) => {
                if is_truthy(self.expr.evaluate(self.env, self.root, v)) {
                    Some(self.parent.new_child_member(v, k))
                } else {
                    self.next()
                }
            }
            None => None,
        }
    }
}

impl<'v> ObjectFilterIter<'v> {
    fn new(
        env: &'static Environment,
        root: &'v Value,
        expr: FilterExpression,
        obj: &'v Map<String, Value>,
        node: Rc<Node<'v>>,
    ) -> Self {
        Self {
            env,
            root,
            expr,
            it: obj.iter(),
            parent: node,
        }
    }
}
