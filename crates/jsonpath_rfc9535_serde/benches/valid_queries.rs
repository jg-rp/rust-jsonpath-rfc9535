#![feature(test)]

extern crate test;

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use jsonpath_rfc9535_serde::find;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use test::Bencher;

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

    #[bench]
    fn bench_cts_queries(b: &mut Bencher) {
        let file = File::open("/tmp/cts.json").expect("could not open CTS file");
        let reader = BufReader::new(file);
        let test_suite: TestSuite =
            serde_json::from_reader(reader).expect("error reading CTS file");

        let valid_queries: Vec<Case> = test_suite
            .tests
            .into_iter()
            .filter(|case| !case.invalid_selector)
            .collect();

        b.iter(|| {
            for case in valid_queries.iter() {
                find(&case.selector, &case.document).unwrap();
            }
        })
    }
}
