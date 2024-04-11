use rust_jsonpath::errors::JSONPathError;
use rust_jsonpath::query::Query;

fn main() -> Result<(), JSONPathError> {
    let q = Query::new("$[\"\\u0013\"]")?;

    println!("{:?}", q);
    println!("{}", q);

    Ok(())
}
