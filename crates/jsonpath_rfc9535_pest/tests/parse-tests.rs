use jsonpath_rfc9535_pest::{errors::JSONPathError, parser::JSONPathParser};

macro_rules! parse_tests {
    ($($name:ident: $value:expr,)*) => {
    mod parse {
        use super::*;
        $(
            #[test]
            fn $name() -> Result<(), JSONPathError> {
                let (input, expected) = $value;
                let query = JSONPathParser::new().parse(input)?;
                assert_eq!(format!("{}", query), expected);
                Ok(())
            }
        )*
        }
    }
}

parse_tests! {
    just_root: ("$", "$"),
    shorthand_name: ("$.foo", "$['foo']"),
    bracketed_name_single_quotes: ("$['foo']", "$['foo']"),
    bracketed_name_double_quotes: ("$[\"foo\"]", "$['foo']"),
    bracketed_index: ("$[1]", "$[1]"),
    slice: ("$[1:-1]", "$[1:-1:1]"),
    slice_with_step: ("$[1:-1:2]", "$[1:-1:2]"),
    slice_with_empty_start: ("$[:-1]", "$[:-1:1]"),
    slice_with_empty_stop: ("$[1:]", "$[1::1]"),
    slice_with_empty_start_and_stop: ("$[:]", "$[::1]"),
    shorthand_wild: ("$.*", "$[*]"),
    bracketed_wild: ("$[*]", "$[*]"),
    multiple_selectors: ("$[1,2]", "$[1, 2]"),
    multiple_selectors_with_slice: ("$[1,5:-1]", "$[1, 5:-1:1]"),
    multiple_selectors_names: ("$[\"some\", 'thing']", "$['some', 'thing']"),
    recursive_shorthand_name: ("$..foo", "$..['foo']"),
    filter_relative_query: ("$[?(@.thing)]", "$[?@['thing']]"),
    filter_root_query: ("$[?($.thing)]", "$[?$['thing']]"),
    filter_compare_eq: ("$.some[?(@.thing == 7)]", "$['some'][?@['thing'] == 7]"),
    filter_compare_ge: ("$.some[?(@.thing >= 7)]", "$['some'][?@['thing'] >= 7]"),
    filter_compare_ne: ("$.some[?(@.thing != 7)]", "$['some'][?@['thing'] != 7]"),
    filter_compare_le: ("$.some[?(@.thing <= 7)]", "$['some'][?@['thing'] <= 7]"),
    filter_compare_lt: ("$.some[?(@.thing < 7)]", "$['some'][?@['thing'] < 7]"),
    filter_boolean_literals: ("$.some[?(true == false)]", "$['some'][?true == false]"),
    filter_null_literal: ("$.some[?(@.thing == null)]", "$['some'][?@['thing'] == null]"),
    filter_string_literal: ("$.some[?(@.thing == 'foo')]", "$['some'][?@['thing'] == 'foo']"),
    filter_integer_literal: ("$.some[?(@.thing == 1)]", "$['some'][?@['thing'] == 1]"),
    filter_float_literal: ("$.some[?(@.thing == 1.1)]", "$['some'][?@['thing'] == 1.1]"),
    filter_logical_not: (
        "$.some[?(@.thing > 1 && !$.other)]",
        "$['some'][?(@['thing'] > 1 && !$['other'])]"
    ),
    filter_grouped_expression: (
        "$.some[?(@.thing > 1 && ($.foo || $.bar))]",
        "$['some'][?(@['thing'] > 1 && ($['foo'] || $['bar']))]"
    ),
    filter_single_quoted_string_with_escape: (
        "$[?@.foo == 'ba\\'r']",
        "$[?@['foo'] == 'ba\'r']"
    ),
    filter_double_quoted_string_with_escape: (
        "$[?@.foo == \"ba\\\"r\"]",
        "$[?@['foo'] == 'ba\"r']"
    ),
    name_selector_escaped_hex: (
        "$[\"\\u263A\"]",
        "$['☺']"
    ),
    name_selector_double_quotes_surrogate_pair: (
        "$[\"\\uD834\\uDD1E\"]",
        "$['𝄞']"
    ),
    name_selector_single_quotes_surrogate_pair: (
        "$['\\uD83D\\uDE00']",
        "$['😀']"
    ),
    function_count: ("$[?count(@..*)>2]", "$[?count(@..[*]) > 2]"),
    filter_and_binds_more_tightly_than_or: ("$[?@.a || @.b && @.b]", "$[?(@['a'] || (@['b'] && @['b']))]"),
}
