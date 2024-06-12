// use std::{fs::File, io::BufReader};

use jsonpath_rfc9535_iter::{jsonpath::find, node::NodeList};
use serde_json::Value;

// fn main() {
//     let file = File::open("/tmp/datasets/citylots.json").expect("could not open data file");
//     let reader = BufReader::new(file);
//     let v: Value = serde_json::from_reader(reader).expect("error reading data file");

//     let q = "$.features..properties";
//     let rv = find(q, &v).unwrap();
//     let values: Vec<&Value> = rv.map(|node| node.value).collect();
//     println!("{:?}", values.len());
// }
fn main() {
    let data = r#"
    {
    "users": [
        {
            "name": "Sue",
            "score": 100
        },
        {
            "name": "Sally",
            "score": 84,
            "admin": false
        },
        {
            "name": "John",
            "score": 86,
            "admin": true
        },
        {
            "name": "Jane",
            "score": 55
        }
    ],
    "moderator": "John"
}"#;

    // Parse the string of data into serde_json::Value.
    let v: Value = serde_json::from_str(data).unwrap();
    let q = "$.users[?@.score > 85]";

    // println!("Q: {:#?}", Query::standard(q));

    let nodes: NodeList = find(q, &v).unwrap().collect();
    let values: Vec<&Value> = nodes.iter().map(|node| node.value).collect();
    let locations: Vec<String> = nodes.iter().map(|node| node.location.clone()).collect();
    println!("{:#?}", values);
    println!("{:#?}", locations);
}
