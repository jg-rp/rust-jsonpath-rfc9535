use jsonpath_rfc9535_singular::find;
use serde_json::Value;

#[test]
fn object_name_from_singular_query() {
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
    let query = "$.a[$.b[1]]";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']['p']");
}

#[test]
fn array_index_from_singular_query() {
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
    let query = "$.a.j[$['c d'].x.y]";
    let nodes = find(query, &value).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes.first().unwrap().path(), "$['a']['j'][1]");
}
