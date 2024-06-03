#![feature(test)]

extern crate test;

#[cfg(test)]
mod tests {
    use lazy_static::lazy_static;
    use std::{fs::File, io::BufReader};

    use jsonpath_rfc9535_serde::{env::Environment, find, Query};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use test::Bencher;

    lazy_static! {
        static ref VALID_QUERIES: Vec<Case> = {
            let file = File::open("../../cts/cts.json").expect("could not open CTS file");
            let reader = BufReader::new(file);
            let test_suite: TestSuite =
                serde_json::from_reader(reader).expect("error reading CTS file");

            let valid_queries: Vec<Case> = test_suite
                .tests
                .into_iter()
                .filter(|case| !case.invalid_selector)
                .collect();

            valid_queries
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

    #[bench]
    fn bench_compile_and_find(b: &mut Bencher) {
        b.iter(|| {
            for case in VALID_QUERIES.iter() {
                find(&case.selector, &case.document).unwrap();
            }
        })
    }

    #[bench]
    fn bench_compile_and_find_values(b: &mut Bencher) {
        b.iter(|| {
            for case in VALID_QUERIES.iter() {
                let _ = find(&case.selector, &case.document)
                    .unwrap()
                    .iter()
                    .map(|node| node.value)
                    .collect::<Vec<&Value>>();
            }
        })
    }

    #[bench]
    fn bench_just_compile(b: &mut Bencher) {
        b.iter(|| {
            for case in VALID_QUERIES.iter() {
                Query::standard(&case.selector).unwrap();
            }
        })
    }

    #[bench]
    fn bench_just_find(b: &mut Bencher) {
        let compiled_queries = VALID_QUERIES
            .iter()
            .map(|case| (Query::standard(&case.selector).unwrap(), &case.document))
            .collect::<Vec<(Query, &Value)>>();

        let env = Environment::new();

        b.iter(|| {
            for (q, v) in compiled_queries.iter() {
                q.find(v, &env).unwrap();
            }
        })
    }
}
