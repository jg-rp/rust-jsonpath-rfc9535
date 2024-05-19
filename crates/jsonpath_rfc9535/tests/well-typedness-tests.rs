use jsonpath_rfc9535::{errors::JSONPathError, ExpressionType, Parser};
use lazy_static::lazy_static;

lazy_static! {
    static ref PARSER: Parser = {
        let mut parser = Parser::new();
        parser.add_function("foo", vec![ExpressionType::Nodes], ExpressionType::Nodes);
        parser.add_function("bar", vec![ExpressionType::Value], ExpressionType::Logical);
        parser.add_function("bn", vec![ExpressionType::Nodes], ExpressionType::Logical);
        parser.add_function("bl", vec![ExpressionType::Logical], ExpressionType::Logical);
        parser
    };
}

macro_rules! assert_valid {
    ($($name:ident: $value:expr,)*) => {
    $(
        #[allow(non_snake_case)]
        #[test]
        fn $name() -> Result<(), JSONPathError> {
            let input = $value;
            PARSER.parse(input)?;
            Ok(())
        }
    )*
    }
}

macro_rules! assert_invalid {
    ($($name:ident: $value:expr,)*) => {
    $(
        #[allow(non_snake_case)]
        #[test]
        #[should_panic]
        fn $name() {
            let input = $value;
            PARSER.parse(input).unwrap();
        }
    )*
    }
}

mod well_typed {
    use super::*;

    assert_valid! {
        length_singular_query_compared: "$[?length(@) < 3]",
        count_non_singular_query_compared: "$[?count(@.*) == 1]",
        nested_function_logicaltype_to_nodestype: "$[?count(foo(@.*)) == 1]",
        match_singular_query_and_string_literal: "$[?match(@.timezone, 'Europe/.*')]",
        value_non_singular_query_param_comparison: "$[?value(@..color) == 'red']",
        function_singular_query_valuetype_to_logicaltype: "$[?bar(@.a)]",
        function_non_singular_query_nodestype_to_logicaltype: "$[?bn(@.*)]",
        function_non_singular_query_logicaltype_to_logicaltype: "$[?bl(@.*)]",
        function_logicaltype_comparison_param: "$[?bl(1==1)]",
        function_valuetype_literal_param: "$[?bar(1)]",
    }

    assert_invalid! {
        length_non_singular_query_compared: "$[?length(@.*) < 3]",
        count_int_literal_compared: "$[?count(1) == 1]",
        match_singular_query_and_string_literal_compared: "$[?match(@.timezone, 'Europe/.*') == true]",
        value_non_singular_query_param: "$[?value(@..color)]",
        function_non_singular_query_valuetype_to_logicaltype: "$[?bar(@.*)]",
        function_logicaltype_literal_param: "$[?bl(1)]",
    }
}
