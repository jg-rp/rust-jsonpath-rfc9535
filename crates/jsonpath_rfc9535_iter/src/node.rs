use serde_json::Value;
pub type NodeList<'v> = Vec<&'v Value>;
pub type NodeIter<'v> = Box<dyn Iterator<Item = &'v Value> + 'v>;
