use jsonpath_rfc9535_serde::jsonpath::find;
use serde_json::Value;

fn main() {
    let data = r#"
        {
            "name": "John Doe",
            "age": 43,
            "phones": [
                "+44 1234567",
                "+44 2345678"
            ],
            "friends": [
                {"name": "foo"},   
                {"name": "bar"},   
                {"name": "baz"}
            ]
        }"#;

    // Parse the string of data into serde_json::Value.
    let v: Value = serde_json::from_str(data).unwrap();
    let q = "$.friends[1:]";

    let rv = find(q, &v).unwrap();
    println!("{:#?}", rv);
}
