// use jsonpath_rfc9535_locations::Query;
use jsonpath_rfc9535_singular::jsonpath::find;
use serde_json::Value;

fn main() {
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

    let v: Value = serde_json::from_str(data).unwrap();
    let q = "$.a.j[$['c d'].x.y]";

    let rv = find(q, &v).unwrap();
    println!("{:?}", rv.len());
    println!("{:?}", rv);
    // let query = Query::standard(q);
    // println!("{:?}", v.is_object());
    // println!("{:?}", query.unwrap().segments.len());
}
