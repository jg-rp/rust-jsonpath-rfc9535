use jsonpath_rfc9535_singular::find;
use serde_json::Value;

#[test]
fn normalized_path() {
    let data = r#"{"a": {"b": [1, 2, 3]}}"#;
    let value: Value = serde_json::from_str(data).unwrap();
    let query = "$.a.b.*";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes.first().unwrap().path(), "$['a']['b'][0]");
    assert_eq!(nodes.last().unwrap().path(), "$['a']['b'][2]");
}

#[test]
fn descendant_normalized_path() {
    let data = r#"{"a": {"b": [1, 2, 3]}}"#;
    let value: Value = serde_json::from_str(data).unwrap();
    let query = "$..b.*";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes.first().unwrap().path(), "$['a']['b'][0]");
    assert_eq!(nodes.last().unwrap().path(), "$['a']['b'][2]");
}
