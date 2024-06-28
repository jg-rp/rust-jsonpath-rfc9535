"""Dump test cases from the JSONPath Compliance Test Suite as calls to valid/invalid macro calls."""

import json
import re
import sys
from operator import itemgetter
from itertools import filterfalse
from typing import NamedTuple
from typing import Optional


class TestCase(NamedTuple):
    name: str
    selector: str
    invalid_selector: Optional[bool]


RE_ESCAPE = re.compile(r"\\u([0-9A-Fa-f]{4})")


def handle_sub(match: re.Match) -> str:
    return rf"\u{{{match.group(1)}}}"


names: dict[str, int] = {}


def snake_name(name: str) -> str:
    name = re.sub(r"[^a-zA-Z0-9_]", "_", name).encode("ascii", errors="ignore").decode()
    cnt = names.get(name, 0)
    names[name] = cnt + 1
    return f"{name}_{cnt}"


def encode_selector(selector: str) -> str:
    # Special case
    if selector == "$.☺":
        return f'"{selector}"'
    # return RE_ESCAPE.sub(handle_sub, json.dumps(selector))
    return json.dumps(selector)


def unique_everseen(iterable, key=None):
    """List unique elements, preserving order. Remember all elements ever seen.

    From https://docs.python.org/3/library/itertools.html
    """
    # unique_everseen('AAAABBBCCDAABBB') → A B C D
    # unique_everseen('ABBcCAD', str.casefold) → A B c D
    seen = set()
    if key is None:
        for element in filterfalse(seen.__contains__, iterable):
            seen.add(element)
            yield element
    else:
        for element in iterable:
            k = key(element)
            if k not in seen:
                seen.add(k)
                yield element


def dedupe(cases: list[tuple[str, str]]) -> list[tuple[str, str]]:
    return list(unique_everseen(cases, key=itemgetter(1)))


def main(path: str) -> None:
    with open(path, "r", encoding="utf-8") as fd:
        data = json.load(fd)

    test_cases = [
        TestCase(t["name"], t["selector"], t.get("invalid_selector", False))
        for t in data["tests"]
    ]

    valid_cases = dedupe(
        [
            (snake_name(t.name), encode_selector(t.selector))
            for t in test_cases
            if not t.invalid_selector
        ]
    )

    valid_cases_str = [f"{name}: {query}" for name, query in valid_cases]

    invalid_cases = dedupe(
        [
            (snake_name(t.name), encode_selector(t.selector))
            for t in test_cases
            if t.invalid_selector
            and "embedded U+" not in t.name  # exclude tricky escapes for now
        ]
    )

    invalid_cases_str = [f"{name}: {query}" for name, query in invalid_cases]

    print(f"assert_valid! {{\n    {',\n    '.join(valid_cases_str)},\n}}")
    print("\n")
    print(f"assert_invalid! {{\n    {',\n    '.join(invalid_cases_str)},\n}}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <path to cts.json>")
        sys.exit(1)

    main(sys.argv[1])
