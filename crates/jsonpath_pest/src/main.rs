use jsonpath_pest::parser::{JSONPath, Rule};
use pest::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rv = JSONPath::parse(Rule::jsonpath, "$.foo[0, 2]")?;
    let tokens = rv.tokens();

    for token in tokens {
        println!("{:?}", token);
    }

    Ok(())
}
