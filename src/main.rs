use rust_jsonpath::errors::JSONPathError;
use rust_jsonpath::Query;

fn main() -> Result<(), JSONPathError> {
    // let parser = Parser::standard();
    // let qq = parser.from_str("$..some[2]")?;

    let q = Query::from_str("$..some[2]")?;

    println!("{:#?}", q);
    println!("{}", q);

    Ok(())
}
