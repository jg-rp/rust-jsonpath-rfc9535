use jsonpath_rfc9535_serde::jsonpath::find;
use serde_json::Value;
use std::{fs::File, io::BufReader};

fn main() {
    // TODO: take CLI args for path and query
    let file = File::open("/tmp/datasets/citylots.json").expect("could not open data file");
    let reader = BufReader::new(file);
    let v: Value = serde_json::from_reader(reader).expect("error reading data file");

    // let q = "$.features..properties";
    // let q = "$.features..properties.BLOCK_NUM";
    let q = "$.features[?@.properties.STREET=='UNKNOWN'].properties.BLOCK_NUM";
    let rv = find(q, &v).unwrap();
    println!("{:?}", rv.len());
    // println!("{:?}", v.is_object())
}
