use rust_jsonpath::Query;

mod errors {
    use super::*;

    #[test]
    #[should_panic(expected = "unknown function 'nosuchthing'")]
    fn unknown_function() {
        Query::standard("$[?nosuchthing()]").unwrap();
    }

    #[test]
    #[should_panic(expected = "count() takes 1 argument but 0 were given")]
    fn not_enough_arguments() {
        Query::standard("$[?count()]").unwrap();
    }

    #[test]
    #[should_panic(expected = "count() takes 1 argument but 2 were given")]
    fn too_many_arguments() {
        Query::standard("$[?count(@.foo, $.bar)]").unwrap();
    }

    #[test]
    #[should_panic(expected = "unbalanced parentheses")]
    fn unbalanced_parens() {
        Query::standard("$[?((@.foo)]").unwrap();
    }

    #[test]
    #[should_panic(expected = "expected a filter expression")]
    fn empty_parens() {
        Query::standard("$[?()]").unwrap();
    }

    #[test]
    #[should_panic(expected = "unclosed bracketed selection")]
    fn unclosed_bracketed_selection() {
        Query::standard("$[1, 3").unwrap();
    }

    #[test]
    #[should_panic(expected = "unclosed bracketed selection")]
    fn unclosed_bracketed_selection_inside_filter() {
        Query::standard("$[?@.a < 1").unwrap();
    }
}
