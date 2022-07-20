use super::error_handling::*;
use std::str;

pub fn bytes_to_string(bytes: &[u8]) -> MyResult<String> {
    str::from_utf8(bytes).map(|s| s.to_string()).my_result()
}

// Turns Option<Vec<T>> into Vec<T>.
// TODO: This should be generalized into Option<Iterator<T>>

pub trait OptionVecTrait {
    type Output;
    fn to_vec(self) -> Vec<Self::Output>;
}

impl <T> OptionVecTrait for Option<Vec<T>> {
    type Output = T;
    fn to_vec(self) -> Vec<T> {
        self.unwrap_or_default()
    }
}
