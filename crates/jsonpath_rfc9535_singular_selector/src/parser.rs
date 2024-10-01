//! A JSONPath parser using [pest].
//!
//! Refer to `jsonpath.pest` and the [pest book]
//!
//! [pest]: https://pest.rs/
//! [pest book]: https://pest.rs/book/

use std::{collections::HashMap, ops::RangeInclusive};

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

use crate::{
    errors::JSONPathError,
    filter::{ComparisonOperator, FilterExpression, LogicalOperator},
    function::{standard_functions, ExpressionType, FunctionSignature},
    query::Query,
    segment::Segment,
    selector::Selector,
    unescape::unescape,
};

#[derive(Parser)]
#[grammar = "jsonpath.pest"]
struct JSONPath;

pub struct JSONPathParser {
    pub index_range: RangeInclusive<i64>,
    pub functions: HashMap<String, FunctionSignature>,
}

impl Default for JSONPathParser {
    fn default() -> Self {
        Self::new()
    }
}

impl JSONPathParser {
    pub fn new() -> Self {
        JSONPathParser {
            index_range: ((-2_i64).pow(53) + 1..=2_i64.pow(53) - 1),
            functions: standard_functions(),
        }
    }

    pub fn parse(&self, query: &str) -> Result<Query, JSONPathError> {
        let segments: Result<Vec<_>, _> = JSONPath::parse(Rule::jsonpath, query)
            .map_err(|err| JSONPathError::syntax(err.to_string()))?
            .map(|segment| self.parse_segment(segment))
            .collect();

        Ok(Query {
            segments: segments?,
        })
    }

    fn parse_segment(&self, segment: Pair<Rule>) -> Result<Segment, JSONPathError> {
        Ok(match segment.as_rule() {
            Rule::child_segment => Segment::Child {
                selectors: self.parse_segment_inner(segment.into_inner().next().unwrap())?,
            },
            Rule::descendant_segment => Segment::Recursive {
                selectors: self.parse_segment_inner(segment.into_inner().next().unwrap())?,
            },
            Rule::name_segment | Rule::index_segment => Segment::Child {
                selectors: vec![self.parse_selector(segment.into_inner().next().unwrap())?],
            },
            Rule::EOI => Segment::Eoi,
            _ => unreachable!(),
        })
    }

    fn parse_segment_inner(&self, segment: Pair<Rule>) -> Result<Vec<Selector>, JSONPathError> {
        Ok(match segment.as_rule() {
            Rule::bracketed_selection => {
                let seg: Result<Vec<_>, _> = segment
                    .into_inner()
                    .map(|selector| self.parse_selector(selector))
                    .collect();
                seg?
            }
            Rule::wildcard_selector => vec![Selector::Wild],
            Rule::member_name_shorthand => vec![Selector::Name {
                // for child_segment
                name: segment.as_str().to_owned(),
            }],
            _ => unreachable!(),
        })
    }

    fn parse_selector(&self, selector: Pair<Rule>) -> Result<Selector, JSONPathError> {
        Ok(match selector.as_rule() {
            Rule::double_quoted => Selector::Name {
                name: unescape(selector.as_str())?,
            },
            Rule::single_quoted => Selector::Name {
                name: unescape(&selector.as_str().replace("\\'", "'"))?,
            },
            Rule::wildcard_selector => Selector::Wild,
            Rule::slice_selector => self.parse_slice_selector(selector)?,
            Rule::index_selector => Selector::Index {
                index: self.parse_i_json_int(selector.as_str())?,
            },
            Rule::filter_selector => self.parse_filter_selector(selector)?,
            Rule::member_name_shorthand => Selector::Name {
                // for name_segment
                name: selector.as_str().to_owned(),
            },
            Rule::singular_query_selector => self.parse_singular_query_selector(selector)?,
            _ => unreachable!(),
        })
    }

    fn parse_slice_selector(&self, selector: Pair<Rule>) -> Result<Selector, JSONPathError> {
        let mut start: Option<i64> = None;
        let mut stop: Option<i64> = None;
        let mut step: Option<i64> = None;

        for i in selector.into_inner() {
            match i.as_rule() {
                Rule::start => start = Some(self.parse_i_json_int(i.as_str())?),
                Rule::stop => stop = Some(self.parse_i_json_int(i.as_str())?),
                Rule::step => step = Some(self.parse_i_json_int(i.as_str())?),
                _ => unreachable!(),
            }
        }

        Ok(Selector::Slice { start, stop, step })
    }

    fn parse_filter_selector(&self, selector: Pair<Rule>) -> Result<Selector, JSONPathError> {
        Ok(Selector::Filter {
            expression: Box::new(
                self.parse_logical_or_expression(selector.into_inner().next().unwrap(), true)?,
            ),
        })
    }

    fn parse_singular_query_selector(
        &self,
        selector: Pair<Rule>,
    ) -> Result<Selector, JSONPathError> {
        let segments: Result<Vec<_>, _> = selector
            .into_inner()
            .map(|segment| self.parse_segment(segment))
            .collect();

        Ok(Selector::SingularQuery {
            query: Box::new(Query {
                segments: segments?,
            }),
        })
    }

    fn parse_logical_or_expression(
        &self,
        expr: Pair<Rule>,
        assert_compared: bool,
    ) -> Result<FilterExpression, JSONPathError> {
        let mut it = expr.into_inner();
        let mut or_expr = self.parse_logical_and_expression(it.next().unwrap(), assert_compared)?;

        if assert_compared {
            self.assert_compared(&or_expr)?;
        }

        for and_expr in it {
            let right = self.parse_logical_and_expression(and_expr, assert_compared)?;
            if assert_compared {
                self.assert_compared(&right)?;
            }
            or_expr = FilterExpression::Logical {
                left: Box::new(or_expr),
                operator: LogicalOperator::Or,
                right: Box::new(right),
            };
        }

        Ok(or_expr)
    }

    fn parse_logical_and_expression(
        &self,
        expr: Pair<Rule>,
        assert_compared: bool,
    ) -> Result<FilterExpression, JSONPathError> {
        let mut it = expr.into_inner();
        let mut and_expr = self.parse_basic_expression(it.next().unwrap())?;

        if assert_compared {
            self.assert_compared(&and_expr)?;
        }

        for basic_expr in it {
            let right = self.parse_basic_expression(basic_expr)?;

            if assert_compared {
                self.assert_compared(&right)?;
            }

            and_expr = FilterExpression::Logical {
                left: Box::new(and_expr),
                operator: LogicalOperator::And,
                right: Box::new(right),
            };
        }

        Ok(and_expr)
    }

    fn parse_basic_expression(&self, expr: Pair<Rule>) -> Result<FilterExpression, JSONPathError> {
        match expr.as_rule() {
            Rule::paren_expr => self.parse_paren_expression(expr),
            Rule::comparison_expr => self.parse_comparison_expression(expr),
            Rule::test_expr => self.parse_test_expression(expr),
            _ => unreachable!(),
        }
    }

    fn parse_paren_expression(&self, expr: Pair<Rule>) -> Result<FilterExpression, JSONPathError> {
        let mut it = expr.into_inner();
        let p = it.next().unwrap();
        match p.as_rule() {
            Rule::logical_not_op => Ok(FilterExpression::Not {
                expression: Box::new(self.parse_logical_or_expression(it.next().unwrap(), true)?),
            }),
            Rule::logical_or_expr => self.parse_logical_or_expression(p, true),
            _ => unreachable!(),
        }
    }

    fn parse_comparison_expression(
        &self,
        expr: Pair<Rule>,
    ) -> Result<FilterExpression, JSONPathError> {
        let mut it = expr.into_inner();
        let left = self.parse_comparable(it.next().unwrap())?;

        let operator = match it.next().unwrap().as_str() {
            "==" => ComparisonOperator::Eq,
            "!=" => ComparisonOperator::Ne,
            "<=" => ComparisonOperator::Le,
            ">=" => ComparisonOperator::Ge,
            "<" => ComparisonOperator::Lt,
            ">" => ComparisonOperator::Gt,
            _ => unreachable!(),
        };

        let right = self.parse_comparable(it.next().unwrap())?;
        self.assert_comparable(&left)?;
        self.assert_comparable(&right)?;

        Ok(FilterExpression::Comparison {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        })
    }

    fn parse_comparable(&self, expr: Pair<Rule>) -> Result<FilterExpression, JSONPathError> {
        Ok(match expr.as_rule() {
            Rule::number => self.parse_number(expr)?,
            Rule::double_quoted => FilterExpression::String {
                value: unescape(expr.as_str())?,
            },
            Rule::single_quoted => FilterExpression::String {
                value: unescape(&expr.as_str().replace("\\'", "'"))?,
            },
            Rule::true_literal => FilterExpression::True,
            Rule::false_literal => FilterExpression::False,
            Rule::null => FilterExpression::Null,
            Rule::rel_singular_query => {
                let segments: Result<Vec<_>, _> = expr
                    .into_inner()
                    .map(|segment| self.parse_segment(segment))
                    .collect();

                FilterExpression::RelativeQuery {
                    query: Box::new(Query {
                        segments: segments?,
                    }),
                }
            }
            Rule::abs_singular_query => {
                let segments: Result<Vec<_>, _> = expr
                    .into_inner()
                    .map(|segment| self.parse_segment(segment))
                    .collect();

                FilterExpression::RootQuery {
                    query: Box::new(Query {
                        segments: segments?,
                    }),
                }
            }
            Rule::function_expr => self.parse_function_expression(expr)?,
            _ => unreachable!(),
        })
    }

    fn parse_number(&self, expr: Pair<Rule>) -> Result<FilterExpression, JSONPathError> {
        if expr.as_str() == "-0" {
            return Ok(FilterExpression::Int { value: 0 });
        }

        // TODO: change pest grammar to indicate positive or negative exponent?
        let mut it = expr.into_inner();
        let mut is_float = false;
        let mut n = it.next().unwrap().as_str().to_string(); // int

        if let Some(pair) = it.next() {
            match pair.as_rule() {
                Rule::frac => {
                    is_float = true;
                    n.push_str(pair.as_str());
                }
                Rule::exp => {
                    let exp_str = pair.as_str();
                    if exp_str.contains('-') {
                        is_float = true;
                    }
                    n.push_str(exp_str);
                }
                _ => unreachable!(),
            }
        }

        if let Some(pair) = it.next() {
            let exp_str = pair.as_str();
            if exp_str.contains('-') {
                is_float = true;
            }
            n.push_str(exp_str);
        }

        if is_float {
            Ok(FilterExpression::Float {
                value: n
                    .parse::<f64>()
                    .map_err(|_| JSONPathError::syntax(String::from("invalid float literal")))?,
            })
        } else {
            Ok(FilterExpression::Int {
                value: n
                    .parse::<f64>()
                    .map_err(|_| JSONPathError::syntax(String::from("invalid integer literal")))?
                    as i64,
            })
        }
    }

    fn parse_test_expression(&self, expr: Pair<Rule>) -> Result<FilterExpression, JSONPathError> {
        let mut it = expr.into_inner();
        let pair = it.next().unwrap();
        Ok(match pair.as_rule() {
            Rule::logical_not_op => FilterExpression::Not {
                expression: Box::new(self.parse_test_expression_inner(it.next().unwrap())?),
            },
            _ => self.parse_test_expression_inner(pair)?,
        })
    }

    fn parse_test_expression_inner(
        &self,
        expr: Pair<Rule>,
    ) -> Result<FilterExpression, JSONPathError> {
        Ok(match expr.as_rule() {
            Rule::rel_query => {
                let segments: Result<Vec<_>, _> = expr
                    .into_inner()
                    .map(|segment| self.parse_segment(segment))
                    .collect();

                FilterExpression::RelativeQuery {
                    query: Box::new(Query {
                        segments: segments?,
                    }),
                }
            }
            Rule::root_query => {
                let segments: Result<Vec<_>, _> = expr
                    .into_inner()
                    .map(|segment| self.parse_segment(segment))
                    .collect();

                FilterExpression::RootQuery {
                    query: Box::new(Query {
                        segments: segments?,
                    }),
                }
            }
            Rule::function_expr => self.parse_function_expression(expr)?,
            _ => unreachable!(),
        })
    }

    fn parse_function_expression(
        &self,
        expr: Pair<Rule>,
    ) -> Result<FilterExpression, JSONPathError> {
        let mut it = expr.into_inner();
        let name = it.next().unwrap().as_str();
        let args: Result<Vec<_>, _> = it.map(|ex| self.parse_function_argument(ex)).collect();

        Ok(FilterExpression::Function {
            name: name.to_string(),
            args: self.assert_well_typed(name, args?)?,
        })
    }

    fn parse_function_argument(&self, expr: Pair<Rule>) -> Result<FilterExpression, JSONPathError> {
        Ok(match expr.as_rule() {
            Rule::number => self.parse_number(expr)?,
            Rule::double_quoted => FilterExpression::String {
                value: unescape(expr.as_str())?,
            },
            Rule::single_quoted => FilterExpression::String {
                value: unescape(&expr.as_str().replace("\\'", "'"))?,
            },
            Rule::true_literal => FilterExpression::True,
            Rule::false_literal => FilterExpression::False,
            Rule::null => FilterExpression::Null,
            Rule::rel_query => {
                let segments: Result<Vec<_>, _> = expr
                    .into_inner()
                    .map(|segment| self.parse_segment(segment))
                    .collect();

                FilterExpression::RelativeQuery {
                    query: Box::new(Query {
                        segments: segments?,
                    }),
                }
            }
            Rule::root_query => {
                let segments: Result<Vec<_>, _> = expr
                    .into_inner()
                    .map(|segment| self.parse_segment(segment))
                    .collect();

                FilterExpression::RootQuery {
                    query: Box::new(Query {
                        segments: segments?,
                    }),
                }
            }
            Rule::logical_or_expr => self.parse_logical_or_expression(expr, false)?,
            Rule::function_expr => self.parse_function_expression(expr)?,
            _ => unreachable!(),
        })
    }

    fn parse_i_json_int(&self, value: &str) -> Result<i64, JSONPathError> {
        let i = value
            .parse::<i64>()
            .map_err(|_| JSONPathError::syntax(format!("index out of range `{}`", value)))?;

        if !self.index_range.contains(&i) {
            return Err(JSONPathError::syntax(format!(
                "index out of range `{}`",
                value
            )));
        }

        Ok(i)
    }
    fn assert_comparable(&self, expr: &FilterExpression) -> Result<(), JSONPathError> {
        // TODO: accept span/position for better errors
        match expr {
            FilterExpression::RelativeQuery { query, .. }
            | FilterExpression::RootQuery { query, .. } => {
                if !query.is_singular() {
                    Err(JSONPathError::typ(String::from(
                        "non-singular query is not comparable",
                    )))
                } else {
                    Ok(())
                }
            }
            FilterExpression::Function { name, .. } => {
                if let Some(FunctionSignature {
                    return_type: ExpressionType::Value,
                    ..
                }) = self.functions.get(name)
                {
                    Ok(())
                } else {
                    Err(JSONPathError::typ(format!(
                        "result of {}() is not comparable",
                        name
                    )))
                }
            }
            _ => Ok(()),
        }
    }

    fn assert_compared(&self, expr: &FilterExpression) -> Result<(), JSONPathError> {
        match expr {
            FilterExpression::Function { name, .. } => {
                if let Some(FunctionSignature {
                    return_type: ExpressionType::Value,
                    ..
                }) = self.functions.get(name)
                {
                    Err(JSONPathError::typ(format!(
                        "result of {}() must be compared",
                        name
                    )))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn assert_well_typed(
        &self,
        func_name: &str,
        args: Vec<FilterExpression>,
    ) -> Result<Vec<FilterExpression>, JSONPathError> {
        // TODO: accept span/position for better errors
        let signature = self
            .functions
            .get(func_name)
            .ok_or_else(|| JSONPathError::name(format!("unknown function `{}`", func_name)))?;

        // correct number of arguments?
        if args.len() != signature.param_types.len() {
            return Err(JSONPathError::typ(format!(
                "{}() takes {} argument{} but {} were given",
                func_name,
                signature.param_types.len(),
                if signature.param_types.len() > 1 {
                    "s"
                } else {
                    ""
                },
                args.len()
            )));
        }

        // correct argument types?
        for (idx, typ) in signature.param_types.iter().enumerate() {
            let arg = &args[idx];
            match typ {
                ExpressionType::Value => {
                    if !self.is_value_type(arg) {
                        return Err(JSONPathError::typ(format!(
                            "argument {} of {}() must be of a 'Value' type",
                            idx + 1,
                            func_name
                        )));
                    }
                }
                ExpressionType::Logical => {
                    if !matches!(
                        arg,
                        FilterExpression::RelativeQuery { .. }
                            | FilterExpression::RootQuery { .. }
                            | FilterExpression::Logical { .. }
                            | FilterExpression::Comparison { .. },
                    ) {
                        return Err(JSONPathError::typ(format!(
                            "argument {} of {}() must be of a 'Logical' type",
                            idx + 1,
                            func_name
                        )));
                    }
                }
                ExpressionType::Nodes => {
                    if !self.is_nodes_type(arg) {
                        return Err(JSONPathError::typ(format!(
                            "argument {} of {}() must be of a 'Nodes' type",
                            idx + 1,
                            func_name
                        )));
                    }
                }
            }
        }

        Ok(args)
    }

    fn is_value_type(&self, expr: &FilterExpression) -> bool {
        // literals are values
        if expr.is_literal() {
            return true;
        }

        match expr {
            FilterExpression::RelativeQuery { query, .. }
            | FilterExpression::RootQuery { query, .. } => {
                // singular queries will be coerced to a value
                query.is_singular()
            }
            FilterExpression::Function { name, .. } => {
                // some functions return a value
                matches!(
                    self.functions.get(name),
                    Some(FunctionSignature {
                        return_type: ExpressionType::Value,
                        ..
                    })
                )
            }
            _ => false,
        }
    }

    fn is_nodes_type(&self, expr: &FilterExpression) -> bool {
        match expr {
            FilterExpression::RelativeQuery { .. } | FilterExpression::RootQuery { .. } => true,
            FilterExpression::Function { name, .. } => {
                matches!(
                    self.functions.get(name),
                    Some(FunctionSignature {
                        return_type: ExpressionType::Nodes,
                        ..
                    })
                )
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pest::Parser;

    macro_rules! assert_valid {
        ($($name:ident: $value:expr,)*) => {
            mod valid {
                use super::*;
                $(
                    #[allow(non_snake_case)]
                    #[test]
                    fn $name() {
                        assert!(JSONPath::parse(Rule::jsonpath, $value).is_ok());
                    }
                )*
            }
        }
    }

    macro_rules! assert_invalid {
        ($($name:ident: $value:expr,)*) => {
            mod invalid {
                use super::*;
                $(
                    #[allow(non_snake_case)]
                    #[test]
                    fn $name() {
                        assert!(JSONPath::parse(Rule::jsonpath, $value).is_err());
                    }
                )*
            }
        }
    }

    // TODO: update CTS

    assert_valid! {
        basic__root_0: "$",
        basic__name_shorthand_0: "$.a",
        basic__name_shorthand__extended_unicode___0: "$.☺",
        basic__name_shorthand__underscore_0: "$._",
        basic__name_shorthand__absent_data_0: "$.c",
        basic__wildcard_shorthand__object_data_0: "$.*",
        basic__wildcard_selector__array_data_0: "$[*]",
        basic__wildcard_shorthand__then_name_shorthand_0: "$.*.a",
        basic__multiple_selectors_0: "$[0,2]",
        basic__multiple_selectors__name_and_index__array_data_0: "$['a',1]",
        basic__multiple_selectors__index_and_slice_0: "$[1,5:7]",
        basic__multiple_selectors__index_and_slice__overlapping_0: "$[1,0:3]",
        basic__multiple_selectors__duplicate_index_0: "$[1,1]",
        basic__multiple_selectors__wildcard_and_index_0: "$[*,1]",
        basic__multiple_selectors__wildcard_and_name_0: "$[*,'a']",
        basic__multiple_selectors__wildcard_and_slice_0: "$[*,0:2]",
        basic__multiple_selectors__multiple_wildcards_0: "$[*,*]",
        basic__descendant_segment__index_0: "$..[1]",
        basic__descendant_segment__name_shorthand_0: "$..a",
        basic__descendant_segment__wildcard_shorthand__array_data_0: "$..*",
        basic__descendant_segment__wildcard_selector__array_data_0: "$..[*]",
        basic__descendant_segment__multiple_selectors_0: "$..['a','d']",
        filter__existence_0: "$[?@.a]",
        filter__equals_string__single_quotes_0: "$[?@.a=='b']",
        filter__equals_numeric_string__single_quotes_0: "$[?@.a=='1']",
        filter__equals_string__double_quotes_0: "$[?@.a==\"b\"]",
        filter__equals_numeric_string__double_quotes_0: "$[?@.a==\"1\"]",
        filter__equals_number_0: "$[?@.a==1]",
        filter__equals_null_0: "$[?@.a==null]",
        filter__equals_true_0: "$[?@.a==true]",
        filter__equals_false_0: "$[?@.a==false]",
        filter__deep_equality__arrays_0: "$[?@.a==@.b]",
        filter__not_equals_string__single_quotes_0: "$[?@.a!='b']",
        filter__not_equals_numeric_string__single_quotes_0: "$[?@.a!='1']",
        filter__not_equals_string__double_quotes_0: "$[?@.a!=\"b\"]",
        filter__not_equals_numeric_string__double_quotes_0: "$[?@.a!=\"1\"]",
        filter__not_equals_number_0: "$[?@.a!=1]",
        filter__not_equals_null_0: "$[?@.a!=null]",
        filter__not_equals_true_0: "$[?@.a!=true]",
        filter__not_equals_false_0: "$[?@.a!=false]",
        filter__less_than_string__single_quotes_0: "$[?@.a<'c']",
        filter__less_than_string__double_quotes_0: "$[?@.a<\"c\"]",
        filter__less_than_number_0: "$[?@.a<10]",
        filter__less_than_null_0: "$[?@.a<null]",
        filter__less_than_true_0: "$[?@.a<true]",
        filter__less_than_false_0: "$[?@.a<false]",
        filter__less_than_or_equal_to_string__single_quotes_0: "$[?@.a<='c']",
        filter__less_than_or_equal_to_string__double_quotes_0: "$[?@.a<=\"c\"]",
        filter__less_than_or_equal_to_number_0: "$[?@.a<=10]",
        filter__less_than_or_equal_to_null_0: "$[?@.a<=null]",
        filter__less_than_or_equal_to_true_0: "$[?@.a<=true]",
        filter__less_than_or_equal_to_false_0: "$[?@.a<=false]",
        filter__greater_than_string__single_quotes_0: "$[?@.a>'c']",
        filter__greater_than_string__double_quotes_0: "$[?@.a>\"c\"]",
        filter__greater_than_number_0: "$[?@.a>10]",
        filter__greater_than_null_0: "$[?@.a>null]",
        filter__greater_than_true_0: "$[?@.a>true]",
        filter__greater_than_false_0: "$[?@.a>false]",
        filter__greater_than_or_equal_to_string__single_quotes_0: "$[?@.a>='c']",
        filter__greater_than_or_equal_to_string__double_quotes_0: "$[?@.a>=\"c\"]",
        filter__greater_than_or_equal_to_number_0: "$[?@.a>=10]",
        filter__greater_than_or_equal_to_null_0: "$[?@.a>=null]",
        filter__greater_than_or_equal_to_true_0: "$[?@.a>=true]",
        filter__greater_than_or_equal_to_false_0: "$[?@.a>=false]",
        filter__exists_and_not_equals_null__absent_from_data_0: "$[?@.a&&@.a!=null]",
        filter__exists_and_exists__data_false_0: "$[?@.a&&@.b]",
        filter__exists_or_exists__data_false_0: "$[?@.a||@.b]",
        filter__and_0: "$[?@.a>0&&@.a<10]",
        filter__or_0: "$[?@.a=='b'||@.a=='d']",
        filter__not_expression_0: "$[?!(@.a=='b')]",
        filter__not_exists_0: "$[?!@.a]",
        filter__nested_0: "$[?@[?@>1]]",
        filter__multiple_selectors_0: "$[?@.a,?@.b]",
        filter__multiple_selectors__comparison_0: "$[?@.a=='b',?@.b=='x']",
        filter__multiple_selectors__overlapping_0: "$[?@.a,?@.d]",
        filter__multiple_selectors__filter_and_index_0: "$[?@.a,1]",
        filter__multiple_selectors__filter_and_wildcard_0: "$[?@.a,*]",
        filter__multiple_selectors__filter_and_slice_0: "$[?@.a,1:]",
        filter__multiple_selectors__comparison_filter__index_and_slice_0: "$[1, ?@.a=='b', 1:]",
        filter__equals_number__zero_and_negative_zero_0: "$[?@.a==-0]",
        filter__equals_number__with_and_without_decimal_fraction_0: "$[?@.a==1.0]",
        filter__equals_number__exponent_0: "$[?@.a==1e2]",
        filter__equals_number__positive_exponent_0: "$[?@.a==1e+2]",
        filter__equals_number__negative_exponent_0: "$[?@.a==1e-2]",
        filter__equals_number__decimal_fraction_0: "$[?@.a==1.1]",
        filter__equals_number__decimal_fraction__exponent_0: "$[?@.a==1.1e2]",
        filter__equals_number__decimal_fraction__positive_exponent_0: "$[?@.a==1.1e+2]",
        filter__equals_number__decimal_fraction__negative_exponent_0: "$[?@.a==1.1e-2]",
        filter__equals__special_nothing_0: "$.values[?length(@.a) == value($..c)]",
        filter__object_data_0: "$[?@<3]",
        filter__and_binds_more_tightly_than_or_0: "$[?@.a || @.b && @.b]",
        filter__left_to_right_evaluation_0: "$[?@.b && @.b || @.a]",
        filter__group_terms__left_0: "$[?(@.a || @.b) && @.a]",
        filter__group_terms__right_0: "$[?@.a && (@.b || @.a)]",
        filter__group_terms__or_before_and_0: "$[?(@.a || @.b) && @.b]",
        index_selector__first_element_0: "$[0]",
        index_selector__second_element_0: "$[1]",
        index_selector__out_of_bound_0: "$[2]",
        index_selector__negative_0: "$[-1]",
        index_selector__more_negative_0: "$[-2]",
        index_selector__negative_out_of_bound_0: "$[-3]",
        name_selector__double_quotes_0: "$[\"a\"]",
        name_selector__double_quotes__absent_data_0: "$[\"c\"]",
        name_selector__double_quotes__embedded_U_0020_0: "$[\" \"]",
        name_selector__double_quotes__escaped_double_quote_0: "$[\"\\\"\"]",
        name_selector__double_quotes__escaped_reverse_solidus_0: "$[\"\\\\\"]",
        name_selector__double_quotes__escaped_solidus_0: "$[\"\\/\"]",
        name_selector__double_quotes__escaped_backspace_0: "$[\"\\b\"]",
        name_selector__double_quotes__escaped_form_feed_0: "$[\"\\f\"]",
        name_selector__double_quotes__escaped_line_feed_0: "$[\"\\n\"]",
        name_selector__double_quotes__escaped_carriage_return_0: "$[\"\\r\"]",
        name_selector__double_quotes__escaped_tab_0: "$[\"\\t\"]",
        name_selector__double_quotes__escaped____upper_case_hex_0: "$[\"\\u263A\"]",
        name_selector__double_quotes__escaped____lower_case_hex_0: "$[\"\\u263a\"]",
        name_selector__double_quotes__surrogate_pair___0: "$[\"\\uD834\\uDD1E\"]",
        name_selector__double_quotes__surrogate_pair___1: "$[\"\\uD83D\\uDE00\"]",
        name_selector__single_quotes_0: "$['a']",
        name_selector__single_quotes__absent_data_0: "$['c']",
        name_selector__single_quotes__embedded_U_0020_0: "$[' ']",
        name_selector__single_quotes__escaped_single_quote_0: "$['\\'']",
        name_selector__single_quotes__escaped_reverse_solidus_0: "$['\\\\']",
        name_selector__single_quotes__escaped_solidus_0: "$['\\/']",
        name_selector__single_quotes__escaped_backspace_0: "$['\\b']",
        name_selector__single_quotes__escaped_form_feed_0: "$['\\f']",
        name_selector__single_quotes__escaped_line_feed_0: "$['\\n']",
        name_selector__single_quotes__escaped_carriage_return_0: "$['\\r']",
        name_selector__single_quotes__escaped_tab_0: "$['\\t']",
        name_selector__single_quotes__escaped____upper_case_hex_0: "$['\\u263A']",
        name_selector__single_quotes__escaped____lower_case_hex_0: "$['\\u263a']",
        name_selector__single_quotes__surrogate_pair___0: "$['\\uD834\\uDD1E']",
        name_selector__single_quotes__surrogate_pair___1: "$['\\uD83D\\uDE00']",
        name_selector__double_quotes__empty_0: "$[\"\"]",
        name_selector__single_quotes__empty_0: "$['']",
        slice_selector__slice_selector_0: "$[1:3]",
        slice_selector__slice_selector_with_step_0: "$[1:6:2]",
        slice_selector__slice_selector_with_everything_omitted__short_form_0: "$[:]",
        slice_selector__slice_selector_with_everything_omitted__long_form_0: "$[::]",
        slice_selector__slice_selector_with_start_omitted_0: "$[:2]",
        slice_selector__slice_selector_with_start_and_end_omitted_0: "$[::2]",
        slice_selector__negative_step_with_default_start_and_end_0: "$[::-1]",
        slice_selector__negative_step_with_default_start_0: "$[:0:-1]",
        slice_selector__negative_step_with_default_end_0: "$[2::-1]",
        slice_selector__larger_negative_step_0: "$[::-2]",
        slice_selector__negative_range_with_default_step_0: "$[-1:-3]",
        slice_selector__negative_range_with_negative_step_0: "$[-1:-3:-1]",
        slice_selector__negative_range_with_larger_negative_step_0: "$[-1:-6:-2]",
        slice_selector__larger_negative_range_with_larger_negative_step_0: "$[-1:-7:-2]",
        slice_selector__negative_from__positive_to_0: "$[-5:7]",
        slice_selector__negative_from_0: "$[-2:]",
        slice_selector__positive_from__negative_to_0: "$[1:-1]",
        slice_selector__negative_from__positive_to__negative_step_0: "$[-1:1:-1]",
        slice_selector__positive_from__negative_to__negative_step_0: "$[7:-5:-1]",
        slice_selector__zero_step_0: "$[1:2:0]",
        slice_selector__empty_range_0: "$[2:2]",
        slice_selector__maximal_range_with_positive_step_0: "$[0:10]",
        slice_selector__maximal_range_with_negative_step_0: "$[9:0:-1]",
        slice_selector__excessively_large_to_value_0: "$[2:113667776004]",
        slice_selector__excessively_small_from_value_0: "$[-113667776004:1]",
        slice_selector__excessively_large_from_value_with_negative_step_0: "$[113667776004:0:-1]",
        slice_selector__excessively_small_to_value_with_negative_step_0: "$[3:-113667776004:-1]",
        slice_selector__excessively_large_step_0: "$[1:10:113667776004]",
        slice_selector__excessively_small_step_0: "$[-1:-10:-113667776004]",
        functions__count__count_function_0: "$[?count(@..*)>2]",
        functions__count__single_node_arg_0: "$[?count(@.a)>1]",
        functions__count__multiple_selector_arg_0: "$[?count(@['a','d'])>1]",
        functions__length__string_data_0: "$[?length(@.a)>=2]",
        functions__length__string_data__unicode_0: "$[?length(@)==2]",
        functions__length__number_arg_0: "$[?length(1)>=2]",
        functions__length__true_arg_0: "$[?length(true)>=2]",
        functions__length__false_arg_0: "$[?length(false)>=2]",
        functions__length__null_arg_0: "$[?length(null)>=2]",
        functions__length__arg_is_a_function_expression_0: "$.values[?length(@.a)==length(value($..c))]",
        functions__length__arg_is_special_nothing_0: "$[?length(value(@.a))>0]",
        functions__match__found_match_0: "$[?match(@.a, 'a.*')]",
        functions__match__double_quotes_0: "$[?match(@.a, \"a.*\")]",
        functions__match__regex_from_the_document_0: "$.values[?match(@, $.regex)]",
        functions__match__don_t_select_match_0: "$[?!match(@.a, 'a.*')]",
        functions__match__non_string_first_arg_0: "$[?match(1, 'a.*')]",
        functions__match__non_string_second_arg_0: "$[?match(@.a, 1)]",
        functions__match__filter__match_function__unicode_char_class__uppercase_0: "$[?match(@, '\\\\p{Lu}')]",
        functions__match__filter__match_function__unicode_char_class_negated__uppercase_0: "$[?match(@, '\\\\P{Lu}')]",
        functions__match__filter__match_function__unicode__surrogate_pair_0: "$[?match(@, 'a.b')]",
        functions__match__arg_is_a_function_expression_0: "$.values[?match(@.a, value($..['regex']))]",
        functions__search__at_the_end_0: "$[?search(@.a, 'a.*')]",
        functions__search__double_quotes_0: "$[?search(@.a, \"a.*\")]",
        functions__search__regex_from_the_document_0: "$.values[?search(@, $.regex)]",
        functions__search__don_t_select_match_0: "$[?!search(@.a, 'a.*')]",
        functions__search__non_string_first_arg_0: "$[?search(1, 'a.*')]",
        functions__search__non_string_second_arg_0: "$[?search(@.a, 1)]",
        functions__search__filter__search_function__unicode_char_class__uppercase_0: "$[?search(@, '\\\\p{Lu}')]",
        functions__search__filter__search_function__unicode_char_class_negated__uppercase_0: "$[?search(@, '\\\\P{Lu}')]",
        functions__search__filter__search_function__unicode__surrogate_pair_0: "$[?search(@, 'a.b')]",
        functions__search__arg_is_a_function_expression_0: "$.values[?search(@, value($..['regex']))]",
        functions__value__single_value_nodelist_0: "$[?value(@.*)==4]",
        whitespace__filter__space_between_question_mark_and_expression_0: "$[? @.a]",
        whitespace__filter__newline_between_question_mark_and_expression_0: "$[?\n@.a]",
        whitespace__filter__tab_between_question_mark_and_expression_0: "$[?\t@.a]",
        whitespace__filter__return_between_question_mark_and_expression_0: "$[?\r@.a]",
        whitespace__filter__space_between_question_mark_and_parenthesized_expression_0: "$[? (@.a)]",
        whitespace__filter__newline_between_question_mark_and_parenthesized_expression_0: "$[?\n(@.a)]",
        whitespace__filter__tab_between_question_mark_and_parenthesized_expression_0: "$[?\t(@.a)]",
        whitespace__filter__return_between_question_mark_and_parenthesized_expression_0: "$[?\r(@.a)]",
        whitespace__filter__space_between_parenthesized_expression_and_bracket_0: "$[?(@.a) ]",
        whitespace__filter__newline_between_parenthesized_expression_and_bracket_0: "$[?(@.a)\n]",
        whitespace__filter__tab_between_parenthesized_expression_and_bracket_0: "$[?(@.a)\t]",
        whitespace__filter__return_between_parenthesized_expression_and_bracket_0: "$[?(@.a)\r]",
        whitespace__filter__space_between_bracket_and_question_mark_0: "$[ ?@.a]",
        whitespace__filter__newline_between_bracket_and_question_mark_0: "$[\n?@.a]",
        whitespace__filter__tab_between_bracket_and_question_mark_0: "$[\t?@.a]",
        whitespace__filter__return_between_bracket_and_question_mark_0: "$[\r?@.a]",
        whitespace__functions__space_between_parenthesis_and_arg_0: "$[?count( @.*)==1]",
        whitespace__functions__newline_between_parenthesis_and_arg_0: "$[?count(\n@.*)==1]",
        whitespace__functions__tab_between_parenthesis_and_arg_0: "$[?count(\t@.*)==1]",
        whitespace__functions__return_between_parenthesis_and_arg_0: "$[?count(\r@.*)==1]",
        whitespace__functions__space_between_arg_and_comma_0: "$[?search(@ ,'[a-z]+')]",
        whitespace__functions__newline_between_arg_and_comma_0: "$[?search(@\n,'[a-z]+')]",
        whitespace__functions__tab_between_arg_and_comma_0: "$[?search(@\t,'[a-z]+')]",
        whitespace__functions__return_between_arg_and_comma_0: "$[?search(@\r,'[a-z]+')]",
        whitespace__functions__space_between_comma_and_arg_0: "$[?search(@, '[a-z]+')]",
        whitespace__functions__newline_between_comma_and_arg_0: "$[?search(@,\n'[a-z]+')]",
        whitespace__functions__tab_between_comma_and_arg_0: "$[?search(@,\t'[a-z]+')]",
        whitespace__functions__return_between_comma_and_arg_0: "$[?search(@,\r'[a-z]+')]",
        whitespace__functions__space_between_arg_and_parenthesis_0: "$[?count(@.* )==1]",
        whitespace__functions__newline_between_arg_and_parenthesis_0: "$[?count(@.*\n)==1]",
        whitespace__functions__tab_between_arg_and_parenthesis_0: "$[?count(@.*\t)==1]",
        whitespace__functions__return_between_arg_and_parenthesis_0: "$[?count(@.*\r)==1]",
        whitespace__functions__spaces_in_a_relative_singular_selector_0: "$[?length(@ .a .b) == 3]",
        whitespace__functions__newlines_in_a_relative_singular_selector_0: "$[?length(@\n.a\n.b) == 3]",
        whitespace__functions__tabs_in_a_relative_singular_selector_0: "$[?length(@\t.a\t.b) == 3]",
        whitespace__functions__returns_in_a_relative_singular_selector_0: "$[?length(@\r.a\r.b) == 3]",
        whitespace__functions__spaces_in_an_absolute_singular_selector_0: "$..[?length(@)==length($ [0] .a)]",
        whitespace__functions__newlines_in_an_absolute_singular_selector_0: "$..[?length(@)==length($\n[0]\n.a)]",
        whitespace__functions__tabs_in_an_absolute_singular_selector_0: "$..[?length(@)==length($\t[0]\t.a)]",
        whitespace__functions__returns_in_an_absolute_singular_selector_0: "$..[?length(@)==length($\r[0]\r.a)]",
        whitespace__operators__space_before____0: "$[?@.a ||@.b]",
        whitespace__operators__newline_before____0: "$[?@.a\n||@.b]",
        whitespace__operators__tab_before____0: "$[?@.a\t||@.b]",
        whitespace__operators__return_before____0: "$[?@.a\r||@.b]",
        whitespace__operators__space_after____0: "$[?@.a|| @.b]",
        whitespace__operators__newline_after____0: "$[?@.a||\n@.b]",
        whitespace__operators__tab_after____0: "$[?@.a||\t@.b]",
        whitespace__operators__return_after____0: "$[?@.a||\r@.b]",
        whitespace__operators__space_before____1: "$[?@.a &&@.b]",
        whitespace__operators__newline_before____1: "$[?@.a\n&&@.b]",
        whitespace__operators__tab_before____1: "$[?@.a\t&&@.b]",
        whitespace__operators__return_before____1: "$[?@.a\r&&@.b]",
        whitespace__operators__space_after____1: "$[?@.a&& @.b]",
        whitespace__operators__space_before____2: "$[?@.a ==@.b]",
        whitespace__operators__newline_before____2: "$[?@.a\n==@.b]",
        whitespace__operators__tab_before____2: "$[?@.a\t==@.b]",
        whitespace__operators__return_before____2: "$[?@.a\r==@.b]",
        whitespace__operators__space_after____2: "$[?@.a== @.b]",
        whitespace__operators__newline_after____2: "$[?@.a==\n@.b]",
        whitespace__operators__tab_after____2: "$[?@.a==\t@.b]",
        whitespace__operators__return_after____2: "$[?@.a==\r@.b]",
        whitespace__operators__space_before____3: "$[?@.a !=@.b]",
        whitespace__operators__newline_before____3: "$[?@.a\n!=@.b]",
        whitespace__operators__tab_before____3: "$[?@.a\t!=@.b]",
        whitespace__operators__return_before____3: "$[?@.a\r!=@.b]",
        whitespace__operators__space_after____3: "$[?@.a!= @.b]",
        whitespace__operators__newline_after____3: "$[?@.a!=\n@.b]",
        whitespace__operators__tab_after____3: "$[?@.a!=\t@.b]",
        whitespace__operators__return_after____3: "$[?@.a!=\r@.b]",
        whitespace__operators__space_before___0: "$[?@.a <@.b]",
        whitespace__operators__newline_before___0: "$[?@.a\n<@.b]",
        whitespace__operators__tab_before___0: "$[?@.a\t<@.b]",
        whitespace__operators__return_before___0: "$[?@.a\r<@.b]",
        whitespace__operators__space_after___0: "$[?@.a< @.b]",
        whitespace__operators__newline_after___0: "$[?@.a<\n@.b]",
        whitespace__operators__tab_after___0: "$[?@.a<\t@.b]",
        whitespace__operators__return_after___0: "$[?@.a<\r@.b]",
        whitespace__operators__space_before___1: "$[?@.b >@.a]",
        whitespace__operators__newline_before___1: "$[?@.b\n>@.a]",
        whitespace__operators__tab_before___1: "$[?@.b\t>@.a]",
        whitespace__operators__return_before___1: "$[?@.b\r>@.a]",
        whitespace__operators__space_after___1: "$[?@.b> @.a]",
        whitespace__operators__newline_after___1: "$[?@.b>\n@.a]",
        whitespace__operators__tab_after___1: "$[?@.b>\t@.a]",
        whitespace__operators__return_after___1: "$[?@.b>\r@.a]",
        whitespace__operators__space_before____4: "$[?@.a <=@.b]",
        whitespace__operators__newline_before____4: "$[?@.a\n<=@.b]",
        whitespace__operators__tab_before____4: "$[?@.a\t<=@.b]",
        whitespace__operators__return_before____4: "$[?@.a\r<=@.b]",
        whitespace__operators__space_after____4: "$[?@.a<= @.b]",
        whitespace__operators__newline_after____4: "$[?@.a<=\n@.b]",
        whitespace__operators__tab_after____4: "$[?@.a<=\t@.b]",
        whitespace__operators__return_after____4: "$[?@.a<=\r@.b]",
        whitespace__operators__space_before____5: "$[?@.b >=@.a]",
        whitespace__operators__newline_before____5: "$[?@.b\n>=@.a]",
        whitespace__operators__tab_before____5: "$[?@.b\t>=@.a]",
        whitespace__operators__return_before____5: "$[?@.b\r>=@.a]",
        whitespace__operators__space_after____5: "$[?@.b>= @.a]",
        whitespace__operators__newline_after____5: "$[?@.b>=\n@.a]",
        whitespace__operators__tab_after____5: "$[?@.b>=\t@.a]",
        whitespace__operators__return_after____5: "$[?@.b>=\r@.a]",
        whitespace__operators__space_between_logical_not_and_test_expression_0: "$[?! @.a]",
        whitespace__operators__newline_between_logical_not_and_test_expression_0: "$[?!\n@.a]",
        whitespace__operators__tab_between_logical_not_and_test_expression_0: "$[?!\t@.a]",
        whitespace__operators__return_between_logical_not_and_test_expression_0: "$[?!\r@.a]",
        whitespace__operators__space_between_logical_not_and_parenthesized_expression_0: "$[?! (@.a=='b')]",
        whitespace__operators__newline_between_logical_not_and_parenthesized_expression_0: "$[?!\n(@.a=='b')]",
        whitespace__operators__tab_between_logical_not_and_parenthesized_expression_0: "$[?!\t(@.a=='b')]",
        whitespace__operators__return_between_logical_not_and_parenthesized_expression_0: "$[?!\r(@.a=='b')]",
        whitespace__selectors__space_between_root_and_bracket_0: "$ ['a']",
        whitespace__selectors__newline_between_root_and_bracket_0: "$\n['a']",
        whitespace__selectors__tab_between_root_and_bracket_0: "$\t['a']",
        whitespace__selectors__return_between_root_and_bracket_0: "$\r['a']",
        whitespace__selectors__space_between_bracket_and_bracket_0: "$['a'] ['b']",
        whitespace__selectors__newline_between_root_and_bracket_1: "$['a'] \n['b']",
        whitespace__selectors__tab_between_root_and_bracket_1: "$['a'] \t['b']",
        whitespace__selectors__return_between_root_and_bracket_1: "$['a'] \r['b']",
        whitespace__selectors__space_between_root_and_dot_0: "$ .a",
        whitespace__selectors__newline_between_root_and_dot_0: "$\n.a",
        whitespace__selectors__tab_between_root_and_dot_0: "$\t.a",
        whitespace__selectors__return_between_root_and_dot_0: "$\r.a",
        whitespace__selectors__space_between_bracket_and_selector_0: "$[ 'a']",
        whitespace__selectors__newline_between_bracket_and_selector_0: "$[\n'a']",
        whitespace__selectors__tab_between_bracket_and_selector_0: "$[\t'a']",
        whitespace__selectors__return_between_bracket_and_selector_0: "$[\r'a']",
        whitespace__selectors__space_between_selector_and_bracket_0: "$['a' ]",
        whitespace__selectors__newline_between_selector_and_bracket_0: "$['a'\n]",
        whitespace__selectors__tab_between_selector_and_bracket_0: "$['a'\t]",
        whitespace__selectors__return_between_selector_and_bracket_0: "$['a'\r]",
        whitespace__selectors__space_between_selector_and_comma_0: "$['a' ,'b']",
        whitespace__selectors__newline_between_selector_and_comma_0: "$['a'\n,'b']",
        whitespace__selectors__tab_between_selector_and_comma_0: "$['a'\t,'b']",
        whitespace__selectors__return_between_selector_and_comma_0: "$['a'\r,'b']",
        whitespace__selectors__space_between_comma_and_selector_0: "$['a', 'b']",
        whitespace__selectors__newline_between_comma_and_selector_0: "$['a',\n'b']",
        whitespace__selectors__tab_between_comma_and_selector_0: "$['a',\t'b']",
        whitespace__selectors__return_between_comma_and_selector_0: "$['a',\r'b']",
        whitespace__slice__space_between_start_and_colon_0: "$[1 :5:2]",
        whitespace__slice__newline_between_start_and_colon_0: "$[1\n:5:2]",
        whitespace__slice__tab_between_start_and_colon_0: "$[1\t:5:2]",
        whitespace__slice__return_between_start_and_colon_0: "$[1\r:5:2]",
        whitespace__slice__space_between_colon_and_end_0: "$[1: 5:2]",
        whitespace__slice__newline_between_colon_and_end_0: "$[1:\n5:2]",
        whitespace__slice__tab_between_colon_and_end_0: "$[1:\t5:2]",
        whitespace__slice__return_between_colon_and_end_0: "$[1:\r5:2]",
        whitespace__slice__space_between_end_and_colon_0: "$[1:5 :2]",
        whitespace__slice__newline_between_end_and_colon_0: "$[1:5\n:2]",
        whitespace__slice__tab_between_end_and_colon_0: "$[1:5\t:2]",
        whitespace__slice__return_between_end_and_colon_0: "$[1:5\r:2]",
        whitespace__slice__space_between_colon_and_step_0: "$[1:5: 2]",
        whitespace__slice__newline_between_colon_and_step_0: "$[1:5:\n2]",
        whitespace__slice__tab_between_colon_and_step_0: "$[1:5:\t2]",
        whitespace__slice__return_between_colon_and_step_0: "$[1:5:\r2]",
    }

    assert_invalid! {
        basic__no_leading_whitespace_0: " $",
        basic__no_trailing_whitespace_0: "$ ",
        basic__name_shorthand__symbol_0: "$.&",
        basic__name_shorthand__number_0: "$.1",
        basic__multiple_selectors__space_instead_of_comma_0: "$[0 2]",
        basic__empty_segment_0: "$[]",
        basic__bald_descendant_segment_0: "$..",
        filter__non_singular_query_in_comparison__slice_0: "$[?@[0:0]==0]",
        filter__non_singular_query_in_comparison__all_children_0: "$[?@[*]==0]",
        filter__non_singular_query_in_comparison__descendants_0: "$[?@..a==0]",
        filter__non_singular_query_in_comparison__combined_0: "$[?@.a[*].a==0]",
        filter__relative_non_singular_query__index__equal_0: "$[?(@[0, 0]==42)]",
        filter__relative_non_singular_query__index__not_equal_0: "$[?(@[0, 0]!=42)]",
        filter__relative_non_singular_query__index__less_or_equal_0: "$[?(@[0, 0]<=42)]",
        filter__relative_non_singular_query__name__equal_0: "$[?(@['a', 'a']==42)]",
        filter__relative_non_singular_query__name__not_equal_0: "$[?(@['a', 'a']!=42)]",
        filter__relative_non_singular_query__name__less_or_equal_0: "$[?(@['a', 'a']<=42)]",
        filter__relative_non_singular_query__combined__equal_0: "$[?(@[0, '0']==42)]",
        filter__relative_non_singular_query__combined__not_equal_0: "$[?(@[0, '0']!=42)]",
        filter__relative_non_singular_query__combined__less_or_equal_0: "$[?(@[0, '0']<=42)]",
        filter__relative_non_singular_query__wildcard__equal_0: "$[?(@.*==42)]",
        filter__relative_non_singular_query__wildcard__not_equal_0: "$[?(@.*!=42)]",
        filter__relative_non_singular_query__wildcard__less_or_equal_0: "$[?(@.*<=42)]",
        filter__relative_non_singular_query__slice__equal_0: "$[?(@[0:0]==42)]",
        filter__relative_non_singular_query__slice__not_equal_0: "$[?(@[0:0]!=42)]",
        filter__relative_non_singular_query__slice__less_or_equal_0: "$[?(@[0:0]<=42)]",
        filter__absolute_non_singular_query__index__equal_0: "$[?($[0, 0]==42)]",
        filter__absolute_non_singular_query__index__not_equal_0: "$[?($[0, 0]!=42)]",
        filter__absolute_non_singular_query__index__less_or_equal_0: "$[?($[0, 0]<=42)]",
        filter__absolute_non_singular_query__name__equal_0: "$[?($['a', 'a']==42)]",
        filter__absolute_non_singular_query__name__not_equal_0: "$[?($['a', 'a']!=42)]",
        filter__absolute_non_singular_query__name__less_or_equal_0: "$[?($['a', 'a']<=42)]",
        filter__absolute_non_singular_query__combined__equal_0: "$[?($[0, '0']==42)]",
        filter__absolute_non_singular_query__combined__not_equal_0: "$[?($[0, '0']!=42)]",
        filter__absolute_non_singular_query__combined__less_or_equal_0: "$[?($[0, '0']<=42)]",
        filter__absolute_non_singular_query__wildcard__equal_0: "$[?($.*==42)]",
        filter__absolute_non_singular_query__wildcard__not_equal_0: "$[?($.*!=42)]",
        filter__absolute_non_singular_query__wildcard__less_or_equal_0: "$[?($.*<=42)]",
        filter__absolute_non_singular_query__slice__equal_0: "$[?($[0:0]==42)]",
        filter__absolute_non_singular_query__slice__not_equal_0: "$[?($[0:0]!=42)]",
        filter__absolute_non_singular_query__slice__less_or_equal_0: "$[?($[0:0]<=42)]",
        filter__equals_number__decimal_fraction__no_fractional_digit_0: "$[?@.a==1.]",
        index_selector__leading_0_0: "$[01]",
        index_selector__leading__0_0: "$[-01]",
        name_selector__double_quotes__invalid_escaped_single_quote_0: "$[\"\\'\"]",
        name_selector__double_quotes__embedded_double_quote_0: "$[\"\"\"]",
        name_selector__double_quotes__incomplete_escape_0: "$[\"\\\"]",
        name_selector__single_quotes__invalid_escaped_double_quote_0: "$['\\\"']",
        name_selector__single_quotes__embedded_single_quote_0: "$[''']",
        name_selector__single_quotes__incomplete_escape_0: "$['\\']",
        slice_selector__too_many_colons_0: "$[1:2:3:4]",
        slice_selector__non_integer_array_index_0: "$[1:2:a]",
        whitespace__functions__space_between_function_name_and_parenthesis_0: "$[?count (@.*)==1]",
        whitespace__functions__newline_between_function_name_and_parenthesis_0: "$[?count\n(@.*)==1]",
        whitespace__functions__tab_between_function_name_and_parenthesis_0: "$[?count\t(@.*)==1]",
        whitespace__functions__return_between_function_name_and_parenthesis_0: "$[?count\r(@.*)==1]",
        whitespace__selectors__space_between_dot_and_name_0: "$. a",
        whitespace__selectors__newline_between_dot_and_name_0: "$.\na",
        whitespace__selectors__tab_between_dot_and_name_0: "$.\ta",
        whitespace__selectors__return_between_dot_and_name_0: "$.\ra",
        whitespace__selectors__space_between_recursive_descent_and_name_0: "$.. a",
        whitespace__selectors__newline_between_recursive_descent_and_name_0: "$..\na",
        whitespace__selectors__tab_between_recursive_descent_and_name_0: "$..\ta",
        whitespace__selectors__return_between_recursive_descent_and_name_0: "$..\ra",
    }
}
