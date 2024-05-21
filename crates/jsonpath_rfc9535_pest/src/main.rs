use jsonpath_rfc9535_pest::parser::JSONPathParser;

fn main() {
    let parser = JSONPathParser::new();
    let rv = parser.parse("$[?@.foo == 'thing' && match(@.bar, 'baz')]");
    match rv {
        Err(err) => print!("{}", err.msg),
        Ok(query) => println!("{}", query),
    }
}
