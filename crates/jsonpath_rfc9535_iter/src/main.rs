use jsonpath_rfc9535_iter::jsonpath::find;
use serde_json::Value;

fn main() {
    let data = r#"
    [
        {
          "a": {
            "b": "foo"
          }
        },
        {}
      ]"#;

    // Parse the string of data into serde_json::Value.
    let v: Value = serde_json::from_str(data).unwrap();
    let q = "$[?length(@ .a .b) == 3]";

    // println!("Q: {:#?}", Query::standard(q));

    let rv = find(q, &v).unwrap();
    let values: Vec<&Value> = rv.collect();
    println!("{:#?}", values);
}
