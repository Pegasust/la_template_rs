use std::error::Error;

pub type AnyErr = Box<dyn Error>;
pub type MyResult<T> = Result<T, AnyErr>;
pub trait MyResultTrait<T> {
    fn my_result(self) -> MyResult<T>;
    fn result_str(self) -> Result<T, String>;
}
impl<T, Err: Into<AnyErr>> MyResultTrait<T> for Result<T, Err> {
    fn my_result(self) -> MyResult<T> {
        self.map_err(|e| e.into())
    }
    fn result_str(self) -> Result<T, String> {
        self.map_err(|e| e.into().to_string())
    }
}

pub fn res_ok<T>(t: T) -> MyResult<T> {
    Ok(t)
}
pub fn res_err<T, E: Into<AnyErr>>(e: E) -> MyResult<T> {
    Err(e.into())
}

pub enum Warning<T> {
    Ok(T),
    Partial(T, AnyErr),
}

impl<T> Warning<T> {
    pub fn from<E: Into<AnyErr>>(partial: T, might_err: Option<E>) -> Self {
        match might_err {
            Some(err) => Warning::Partial(partial, err.into()),
            None => Warning::Ok(partial),
        }
    }
}
