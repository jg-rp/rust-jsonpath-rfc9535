use jsonpath_rfc9535_singular::find;
use serde_json::Value;

#[test]
fn just_a_name() {
    let data = r#"
    {
        "a": "b"
    }"#;

    let value: Value = serde_json::from_str(data).unwrap();
    let query = "a";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']");
    assert_eq!(nodes.first().unwrap().value.as_str().unwrap(), "b");
}

#[test]
fn start_with_bracketed_name() {
    let data = r#"
    {
        "a": "b"
    }"#;

    let value: Value = serde_json::from_str(data).unwrap();
    let query = "['a']";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']");
    assert_eq!(nodes.first().unwrap().value.as_str().unwrap(), "b");
}

#[test]
fn start_with_bracketed_names() {
    let data = r#"
    {
        "a": "b"
    }"#;

    let value: Value = serde_json::from_str(data).unwrap();
    let query = "['a', 'b']";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']");
    assert_eq!(nodes.first().unwrap().value.as_str().unwrap(), "b");
}

#[test]
fn start_with_bracketed_wildcard() {
    let data = r#"
    {
        "a": "b"
    }"#;

    let value: Value = serde_json::from_str(data).unwrap();
    let query = "[*]";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']");
    assert_eq!(nodes.first().unwrap().value.as_str().unwrap(), "b");
}

#[test]
fn singular_query_selector() {
    let data = r#"
    {
        "a": {
            "j": [1, 2, 3],
            "p": {
            "q": [4, 5, 6]
            }
        },
        "b": ["j", "p", "q"],
        "c d": {
            "x": {
            "y": 1
            }
        }
    }"#;

    let value: Value = serde_json::from_str(data).unwrap();
    let query = "a[b[1]]";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']['p']");
}
