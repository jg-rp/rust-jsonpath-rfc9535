use jsonpath_rfc9535_pest_recursive::JSONPathParser;

fn main() {
    let p = JSONPathParser::new();
    let q = "$..foo[0]";
    match p.parse(q) {
        Err(err) => print!("{}", err.msg),
        Ok(query) => {
            println!("{}", query);
            println!("{:#?}", query);
        }
    }
}
