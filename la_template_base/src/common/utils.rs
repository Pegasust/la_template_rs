use super::error_handling::*;
use std::str;

pub fn bytes_to_string(bytes: &[u8]) -> MyResult<String> {
    str::from_utf8(bytes).map(|s| s.to_string()).my_result()
}
