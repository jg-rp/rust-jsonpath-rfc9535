use std::{iter, rc::Rc, slice::Iter, vec::IntoIter};

use serde_json::{Map, Value};

use crate::{
    env::Environment,
    filter::{is_truthy, FilterExpression},
    node::NodeIter,
    segment::{visit_iter, Segment},
    selector::{norm_index, slice, Selector},
    Query,
};

pub struct QueryIter<'v> {
    it: NodeIter<'v>, // Just SegmentIter
}

impl<'v> Iterator for QueryIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

impl<'v> QueryIter<'v> {
    pub fn new(env: Rc<Environment>, root: &'v Value, query: Query) -> Self {
        let init = SegmentIter {
            selectors: vec![].into_iter(),
            it: Box::new(iter::once(root)),
        };

        let it = query
            .segments
            .into_iter()
            .filter(|s| !matches!(s, Segment::Eoi {}))
            .fold(init, |values, segment| {
                SegmentIter::new(env.clone(), root, segment, Box::new(values))
            });

        Self { it: Box::new(it) }
    }
}

pub struct SegmentIter<'v> {
    selectors: IntoIter<SelectorIter<'v>>,
    it: NodeIter<'v>,
}

impl<'v> Iterator for SegmentIter<'v> {
    type Item = &'v Value;
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
        env: Rc<Environment>,
        root: &'v Value,
        segment: Segment,
        values: NodeIter<'v>,
    ) -> Self {
        let mut its: Vec<SelectorIter<'v>> = Vec::new();
        match segment {
            Segment::Child { ref selectors } => {
                for value in values {
                    for selector in selectors {
                        its.push(SelectorIter::new(
                            env.clone(),
                            root,
                            selector.clone(),
                            value,
                        ))
                    }
                }
            }
            Segment::Recursive { ref selectors } => {
                for value in values {
                    for val in visit_iter(value) {
                        for selector in selectors {
                            its.push(SelectorIter::new(env.clone(), root, selector.clone(), val))
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
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

impl<'v> SelectorIter<'v> {
    fn new(env: Rc<Environment>, root: &'v Value, selector: Selector, value: &'v Value) -> Self {
        let it: NodeIter<'v> = match selector {
            Selector::Name { name } => {
                if let Some(v) = value.get(name) {
                    Box::new(iter::once(v))
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Index { index } => {
                if let Some(array) = value.as_array() {
                    let norm = norm_index(index, array.len());
                    if let Some(v) = array.get(norm) {
                        Box::new(iter::once(v))
                    } else {
                        Box::new(iter::empty())
                    }
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Slice { start, stop, step } => {
                if let Some(array) = value.as_array() {
                    // TODO: lazy slice
                    Box::new(slice(array, start, stop, step).into_iter().map(|(_, v)| v))
                } else {
                    Box::new(iter::empty())
                }
            }
            Selector::Wild {} => match value {
                Value::Array(arr) => Box::new(arr.iter().enumerate().map(|(_i, v)| v)),
                Value::Object(obj) => Box::new(obj.iter().map(|(_k, v)| v)),
                _ => Box::new(iter::empty()),
            },
            Selector::Filter { expression } => match value {
                Value::Array(arr) => {
                    Box::new(ArrayFilterIter::new(env.clone(), root, *expression, arr))
                }
                Value::Object(obj) => {
                    Box::new(ObjectFilterIter::new(env.clone(), root, *expression, obj))
                }
                _ => Box::new(iter::empty()),
            },
        };

        Self { it }
    }
}

pub struct ArrayFilterIter<'v> {
    env: Rc<Environment>,
    root: &'v Value,
    expr: FilterExpression,
    it: Iter<'v, Value>,
}

impl<'v> Iterator for ArrayFilterIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some(value) => {
                if is_truthy(self.expr.evaluate(self.env.clone(), self.root, value)) {
                    Some(value)
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
        env: Rc<Environment>,
        root: &'v Value,
        expr: FilterExpression,
        arr: &'v Vec<Value>,
    ) -> Self {
        Self {
            env: env.clone(),
            root,
            expr,
            it: arr.iter(),
        }
    }
}

pub struct ObjectFilterIter<'v> {
    env: Rc<Environment>,
    root: &'v Value,
    expr: FilterExpression,
    it: serde_json::map::Iter<'v>,
}

impl<'v> Iterator for ObjectFilterIter<'v> {
    type Item = &'v Value;
    fn next(&mut self) -> Option<Self::Item> {
        match self.it.next() {
            Some((_k, v)) => {
                if is_truthy(self.expr.evaluate(self.env.clone(), self.root, v)) {
                    Some(v)
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
        env: Rc<Environment>,
        root: &'v Value,
        expr: FilterExpression,
        obj: &'v Map<String, Value>,
    ) -> Self {
        Self {
            env,
            root,
            expr,
            it: obj.iter(),
        }
    }
}
