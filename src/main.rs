use jsonpath_rfc9535::{errors::JSONPathError, ExpressionType, Parser};

fn main() -> Result<(), JSONPathError> {
    let mut parser = Parser::new();

    parser.add_function(
        "foo",
        vec![ExpressionType::Value, ExpressionType::Nodes],
        ExpressionType::Logical,
    );

    let q = parser.parse("$.some[?foo('7', @.thing)][1, 4]")?;

    // println!("{:#?}", q);
    println!("{}", q);

    Ok(())
}
