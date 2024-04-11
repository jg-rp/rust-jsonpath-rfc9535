import json
import re
import sys
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

    name += str(cnt)
    return name


def encode_selector(selector: str) -> str:
    # Special case
    if selector == "$.☺":
        return f'"{selector}"'
    # return RE_ESCAPE.sub(handle_sub, json.dumps(selector))
    return json.dumps(selector)


def main(path: str) -> None:
    with open(path, "r", encoding="utf-8") as fd:
        data = json.load(fd)

    test_cases = [
        TestCase(t["name"], t["selector"], t.get("invalid_selector", False))
        for t in data["tests"]
    ]

    valid_cases = [
        f"{snake_name(t.name)}: {encode_selector(t.selector)}"
        for t in test_cases
        if not t.invalid_selector
    ]

    invalid_cases = [
        f"{snake_name(t.name)}: {encode_selector(t.selector)}"
        for t in test_cases
        if t.invalid_selector
        and "embedded U+" not in t.name  # exclude tricky escapes for now
    ]

    print(f"assert_valid! {{\n    {',\n    '.join(valid_cases)},\n}}")
    print("\n")
    print(f"assert_invalid! {{\n    {',\n    '.join(invalid_cases)},\n}}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <path to cts.json>")
        sys.exit(1)

    main(sys.argv[1])
