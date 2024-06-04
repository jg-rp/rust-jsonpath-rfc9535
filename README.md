# Rust JSONPath RFC 9535

An exploration of JSONPath parsing and evaluation in Rust with Python bindings in mind.

- `crates/jsonpath_rfc9535` is a hand-crafted lexer and parser for JSONPath producing a JSON implementation agnostic abstract syntax tree, following the JSONPath model described in RFC 9535.
- `crates/jsonpath_rfc9535_pest` is a [pest](https://github.com/pest-parser)-based JSONPath parser, producing a similar AST to the hand-crafted parser.
- `crates/jsonpath_rfc9535_pest_recursive` is the pest parser producing an AST structured with recursive segments rather than a vector of segments. This structure is inspired by the stalled [jsonpath-reference-implementation](https://github.com/jsonpath-standard/jsonpath-reference-implementation).
- `crates/jsonpath_rfc9535_serde` implements JSONPath evaluation using serde JSON, based on the pest parser.

## Hand-crafted parser

### Standard queries

To parse a JSONPath expression that is limited to standard [function extensions], use `Query::standard`.

```rust
use jsonpath_rfc9535::{errors::JSONPathError, Query};

fn main() -> Result<(), JSONPathError> {
    let q = Query::standard("$..foo[0]")?;
    println!("{:#?}", q);
    Ok(())
}
```

Debug output from the example above shows this syntax tree:

```text
Query {
    segments: [
        Recursive {
            span: (
                1,
                3,
            ),
            selectors: [
                Name {
                    span: (
                        3,
                        6,
                    ),
                    name: "foo",
                },
            ],
        },
        Child {
            span: (
                6,
                7,
            ),
            selectors: [
                Index {
                    span: (
                        7,
                        8,
                    ),
                    index: 0,
                },
            ],
        },
    ],
}
```

### Function extensions

Register [function extensions] with a new `Parser` by calling `Parser::add_function`,
then use `Parser::parse` to create a new `Query`.

```rust
use jsonpath_rfc9535::{errors::JSONPathError, ExpressionType, Parser};

fn main() -> Result<(), JSONPathError> {
    let mut parser = Parser::new();

    parser.add_function(
        "foo",
        vec![ExpressionType::Value, ExpressionType::Nodes],
        ExpressionType::Logical,
    );

    let q = parser.parse("$.some[?foo('7', @.thing)][1, 4]")?;

    println!("{:?}", q);
    Ok(())
}
```

Note that a `Query` is displayed in its canonical form when printed.

```text
$['some'][?foo("7", @['thing'])][1, 4]
```

Without registering a signature for `foo`, we would get a `JSONPathError` with
`kind` set to `JSONPathErrorType::NameError`.

```text
Error: JSONPathError { kind: NameError, msg: "unknown function `foo`", span: (8, 11) }
```

[function extensions]: https://datatracker.ietf.org/doc/html/rfc9535#name-function-extensions

## Pest-based parser

TODO:

## Recursive pest-based parser

`crates/jsonpath_rfc9535_pest_recursive` is the pest parser producing an AST structured with recursive segments rather than a vector of segments.

```rust
use jsonpath_rfc9535_pest_recursive::JSONPathParser;

fn main() {
    let p = JSONPathParser::new();
    let q = "$..foo[0]";
    match p.parse(q) {
        Err(err) => print!("{}", err.msg),
        Ok(query) => {
            println!("{:#?}", query);
        }
    }
}
```

```text
Query {
    ast: Child {
        left: Recursive {
            left: Root,
            selectors: [
                Name {
                    name: "foo",
                },
            ],
        },
        selectors: [
            Index {
                index: 0,
            },
        ],
    },
}
```

## Performance Notes

Without attempting to optimize the grammar, the pest-based parser benchmarks at 164,385 ns/iter, vs 74,718 ns/iter for the hand-crafted parser, and it is marginally faster to produce an AST with recursive segments rather than a vector of segments.

When benchmarking [JPQ](https://github.com/jg-rp/jpq) with the pest parser, this translates to a slowdown of between 0.03 and 0.04 seconds (409 queries repeated 100 times) during the compile phase. This seems like a good tradeoff.

Benchmarking `crates/jsonpath_rfc9535_serde` on an M2 Mac Mini we get the following results:

```
test tests::bench_compile_and_find        ... bench:     789,637 ns/iter (+/- 33,289)
test tests::bench_compile_and_find_values ... bench:     793,870 ns/iter (+/- 5,057)
test tests::bench_just_compile            ... bench:     526,270 ns/iter (+/- 2,064)
test tests::bench_just_find               ... bench:     243,718 ns/iter (+/- 1,709)
test tests::bench_just_find_loop          ... bench:     235,477 ns/iter (+/- 1,893)
```

Which shows a 2x performance improvement using Serde JSON over JPQ.

The last two lines show an insignificant difference in performance between code that uses explicit `for` loops and vectors to collect nodes vs extensive use of iterator adapters and `collect()`.

## Contributing

### Development

We're using a Rust [workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) with `crates/jsonpath_rfc9535` being the default workspace member. Use the `-p` or `--package` option when using `cargo` to select a different crate.

Run tests with cargo.

```shell
$ cargo test
```

Lint with clippy.

```shell
$ cargo clippy
```

Build with cargo.

```shell
$ cargo build
```

Check test coverage with [cargo-llvm-cov](https://lib.rs/crates/cargo-llvm-cov):

```shell
$ cargo llvm-cov
```

Or, write an HTML report to `target/llvm-cov/html`:

```shell
$ cargo llvm-cov --html
```
