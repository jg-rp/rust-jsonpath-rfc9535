use rust_jsonpath::errors::JSONPathError;
use rust_jsonpath::Query;

fn main() -> Result<(), JSONPathError> {
    // let parser = Parser::new();
    // let qq = parser.parse("$..some[2]")?;

    let q = Query::standard("$[?((@.foo))]")?;

    println!("{:?}", q);
    println!("{}", q);

    Ok(())
}
