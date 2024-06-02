use jsonpath_rfc9535_serde::jsonpath::find;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, error::Error, fs::File, io::BufReader};

lazy_static! {
    static ref SKIP: HashSet<String> = {
        let mut skip = HashSet::new();
        skip.insert("functions, match, dot matcher on \\u2028".to_owned());
        skip.insert("functions, match, dot matcher on \\u2029".to_owned());
        skip.insert("functions, search, dot matcher on \\u2028".to_owned());
        skip.insert("functions, search, dot matcher on \\u2029".to_owned());
        skip
    };
}

#[derive(Serialize, Deserialize)]
struct TestSuite {
    tests: Vec<Case>,
}

#[derive(Serialize, Deserialize)]
struct Case {
    name: String,
    selector: String,

    #[serde(default)]
    document: Value,

    #[serde(default)]
    result: Vec<Value>,

    #[serde(default)]
    results: Vec<Vec<Value>>,

    #[serde(default)]
    invalid_selector: bool,
}

fn flame() -> Result<(), Box<dyn Error>> {
    let file = File::open("/tmp/cts.json")?;
    let reader = BufReader::new(file);
    let test_suite: TestSuite = serde_json::from_reader(reader)?;

    let valid_queries: Vec<Case> = test_suite
        .tests
        .into_iter()
        .filter(|case| !case.invalid_selector)
        .collect();

    for _ in 1..100 {
        for case in valid_queries.iter() {
            find(&case.selector, &case.document).unwrap();
        }
    }

    Ok(())
}

fn main() {
    // let data = r#"
    // [
    //     {
    //       "a": "ab"
    //     }
    //   ]"#;

    // // Parse the string of data into serde_json::Value.
    // let v: Value = serde_json::from_str(data).unwrap();
    // let q = "$[?match(@.a, 'a.*')]";

    // // println!("Q: {:#?}", Query::standard(q));

    // let rv = find(q, &v).unwrap();
    // println!("{:#?}", rv);
    flame().unwrap()
}
