use super::error_handling::*;
use std::str;

pub fn bytes_to_string<B>(bytes: B) -> MyResult<String> 
    where B: AsRef<[u8]>
{
    str::from_utf8(bytes.as_ref()).map(|s| s.to_string()).my_result()
}
pub trait BytesToStringExt: Sized+AsRef<[u8]> {
    fn to_str(self) -> MyResult<String> {
        bytes_to_string(self)
    }
}
impl <B: Sized+AsRef<[u8]>> BytesToStringExt for B {}

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
