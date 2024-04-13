use rust_jsonpath::errors::JSONPathError;
use rust_jsonpath::Query;

fn main() -> Result<(), JSONPathError> {
    let q = Query::new("$..some[2]")?;

    println!("{:#?}", q);
    println!("{}", q);

    Ok(())
}
