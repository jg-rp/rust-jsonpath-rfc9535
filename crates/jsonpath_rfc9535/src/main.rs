use jsonpath_rfc9535::{errors::JSONPathError, Parser};

fn main() -> Result<(), JSONPathError> {
    let parser = Parser::new();
    let q = parser.parse("$.some[?@.thing]")?;
    println!("{:#?}", q);
    Ok(())
}
