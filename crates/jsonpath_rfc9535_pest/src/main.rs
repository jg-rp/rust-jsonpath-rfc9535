use jsonpath_rfc9535_pest::JSONPathParser;

fn main() {
    let p = JSONPathParser::new();
    let q = "$.foo.bar";
    match p.parse(q) {
        Err(err) => print!("{}", err.msg),
        Ok(query) => println!("{:#?}", query),
    }
}
