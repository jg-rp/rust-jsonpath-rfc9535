use std::{collections::HashSet, error::Error, fs::File, io::BufReader};

use jsonpath_rfc9535_locations::{jsonpath::find, Query};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[test]
fn compliance() -> Result<(), Box<dyn Error>> {
    // Path is relative to the crate root, crates/jsonpath_rfc9535_serde in this case.
    let file = File::open("../../cts/cts.json")?;
    let reader = BufReader::new(file);
    let test_suite: TestSuite = serde_json::from_reader(reader)?;

    for case in test_suite.tests {
        if SKIP.contains(case.name.as_str()) {
            println!("SKIPPING {}", case.name);
            continue;
        }

        println!("{}", case.name);
        if case.invalid_selector {
            assert!(
                Query::standard(&case.selector).is_err(),
                "{} did not fail",
                case.name
            );
        } else if case.results.len() > 0 {
            ()
        } else {
            let rv = find(&case.selector, &case.document)?;
            let values: Vec<Value> = rv.iter().map(|n| n.value.clone()).collect();
            assert_eq!(values, case.result, "{}: {}", case.name, case.selector);
        }
    }

    Ok(())
}
