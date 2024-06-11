use std::{fs::File, io::BufReader};

use jsonpath_rfc9535_iter::jsonpath::find;
use serde_json::Value;

fn main() {
    let file = File::open("/tmp/datasets/citylots.json").expect("could not open data file");
    let reader = BufReader::new(file);
    let v: Value = serde_json::from_reader(reader).expect("error reading data file");

    let q = "$.features..properties";
    let rv = find(q, &v).unwrap();
    let values: Vec<&Value> = rv.collect();
    println!("{:?}", values.len());
}
