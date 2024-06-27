// use jsonpath_rfc9535_locations::Query;
use jsonpath_rfc9535_locations::jsonpath::find;
use serde_json::Value;

fn main() {
    let data = r#"
        [
        0,
        1,
        2,
        3
      ]"#;

    let v: Value = serde_json::from_str(data).unwrap();
    let q = "$[::-1]";

    let rv = find(q, &v).unwrap();
    println!("{:?}", rv.len());
    println!("{:?}", rv);
    // let query = Query::standard(q);
    // println!("{:?}", v.is_object());
    // println!("{:?}", query.unwrap().segments.len());
}
