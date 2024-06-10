# A lazy implementation of RFC 9535 JSONPath

Experimental JSONPath evaluation with Rust iterators and Serde JSON. This implementation produces an iterator over JSON _values_ (not _nodes_) and is infallible (it does not return a `Result`).
