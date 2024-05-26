use jsonpath_rfc9535_pest::Query;

fn main() {
    let q = "$[?@.foo == 'thing' && match(@.bar, 'baz')]";
    match Query::standard(q) {
        Err(err) => print!("{}", err.msg),
        Ok(query) => println!("{}", query),
    }
}
