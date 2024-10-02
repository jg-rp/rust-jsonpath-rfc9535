use core::str;

use crate::errors::JSONPathError;

pub fn unescape(value: &str) -> Result<String, JSONPathError> {
    let bytes = value.as_bytes();
    let length = bytes.len();
    let mut rv: Vec<u8> = Vec::new();
    let mut index: usize = 0;
    let mut code_point: u32;

    while index < length {
        let b = bytes[index];
        if b == b'\\' {
            index += 1;
            match bytes[index] {
                b'"' => rv.push(b'"'),
                b'\\' => rv.push(b'\\'),
                b'/' => rv.push(b'/'),
                b'b' => rv.push(b'\x08'),
                b'f' => rv.push(b'\x0C'),
                b'n' => rv.push(b'\n'),
                b'r' => rv.push(b'\r'),
                b't' => rv.push(b'\t'),
                b'u' => {
                    (code_point, index) = decode_hex_char(bytes, index)?;
                    let mut x = encode_code_point(code_point)?;
                    rv.append(&mut x);
                }
                _ => return Err(JSONPathError::syntax("unknown escape sequence".to_owned())),
            }
        } else {
            rv.push(b);
        }
        index += 1;
    }

    return Ok(String::from_utf8(rv).unwrap());
}

fn decode_hex_char(bytes: &[u8], index: usize) -> Result<(u32, usize), JSONPathError> {
    let length = bytes.len();
    let mut index = index;

    if index + 4 >= length {
        return Err(JSONPathError::syntax(
            "incomplete escape sequence".to_owned(),
        ));
    }

    index = index + 1; // move past 'u'
    let mut code_point = parse_hex_digits(&bytes[index..index + 4])?;

    if is_low_surrogate(code_point) {
        return Err(JSONPathError::syntax(
            "unexpected low surrogate code point".to_owned(),
        ));
    }

    if is_high_surrogate(code_point) {
        if !(index + 9 < length && bytes[index + 4] == b'\\' && bytes[index + 5] == b'u') {
            return Err(JSONPathError::syntax(
                "incomplete escape sequence".to_owned(),
            ));
        }

        let low_surrogate = parse_hex_digits(&bytes[index + 6..index + 10])?;

        if !is_low_surrogate(low_surrogate) {
            return Err(JSONPathError::syntax("unexpected code point".to_owned()));
        }

        code_point = 0x10000 + (((code_point & 0x03FF) << 10) | (low_surrogate & 0x03FF));
        return Ok((code_point, index + 9));
    }

    Ok((code_point, index + 3))
}

fn parse_hex_digits(digits: &[u8]) -> Result<u32, JSONPathError> {
    let s = str::from_utf8(digits).unwrap();
    u32::from_str_radix(s, 16)
        .map_err(|_| JSONPathError::syntax("invalid escape sequence".to_owned()))
}

fn encode_code_point(code_point: u32) -> Result<Vec<u8>, JSONPathError> {
    if code_point < 0x1F {
        Err(JSONPathError::syntax("invalid character".to_owned()))
    } else {
        // TODO: better
        let mut buf = [0; 4];
        let rv = char::from_u32(code_point).unwrap().encode_utf8(&mut buf);
        Ok(rv.as_bytes().to_owned())
    }
}

fn is_high_surrogate(code_point: u32) -> bool {
    code_point >= 0xD800 && code_point <= 0xDBFF
}

fn is_low_surrogate(code_point: u32) -> bool {
    code_point >= 0xDC00 && code_point <= 0xDFFF
}
