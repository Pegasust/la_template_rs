mod common;
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{BufRead, Seek},
};

use common::{MyResult, Warning};
use enum_dispatch::enum_dispatch;
use serde_json::Value;
use simple_error::simple_error;
pub struct TemplateArgs<V: ReadVariable, T: BufRead + Seek> {
    var: V,
    template: T,
}

impl<V: ReadVariable, T: BufRead + Seek> TemplateArgs<V, T> {
    pub fn new(var: V, template: T) -> Self {
        Self { var, template }
    }
    pub fn generate_template(self) -> MyResult<String> {
        self.generate_template_partial()
            .and_then(|warn| match warn {
                Warning::Ok(s) => Ok(s),
                Warning::Partial(s, err) => {
                    Err(simple_error!("Failed: {:?};\nPartial:{}\n", s, err).into())
                }
            })
    }
    pub fn generate_template_partial(self) -> MyResult<Warning<String>> {
        unimplemented!()
    }
}

#[enum_dispatch]
pub trait ReadVariable {
    fn _get_defn<'a>(&'a self, key: &str) -> MyResult<Cow<'a, str>>;
    fn get_defn<AnyStr: AsRef<str>>(&self, key: AnyStr) -> MyResult<Cow<'_, str>> {
        self._get_defn(key.as_ref())
    }
}
#[enum_dispatch(ReadVariable)]
pub enum VariableMap {
    HashMapStd(HashMap<String, String>),
    SerdeJsonValue(Value),
}

// #[enum_dispatch]
pub trait CanParse: BufRead + Seek {}
impl<T: BufRead + Seek> CanParse for T {}

#[enum_dispatch(Seek, BufRead, CanParse)]
pub enum Template {}

impl VariableMap {
    pub fn json_str<AnyStr: AsRef<str>>(s: AnyStr) -> MyResult<VariableMap> {
        let v: Value = serde_json::from_str(s.as_ref())?;
        Ok(v.into())
    }
}
impl ReadVariable for HashMap<String, String> 
{
    fn _get_defn<'a>(&'a self, key: &str) -> MyResult<Cow<'a, str>> {
        self.get(key)
            .ok_or_else(|| simple_error!("Variable {} not found.", key).into())
            .map(Cow::from)
    }
}
impl ReadVariable for Value {
    fn _get_defn<'a>(&'a self, key: &str) -> MyResult<Cow<'a, str>> {
        self.as_object()
            .ok_or_else(|| simple_error!("Value {:?} cannot be parsed as obj", self))
            .and_then(|m| {
                m.get(key).ok_or_else(|| {
                    simple_error!("Value {:?} has no variable named {key} defined.", self)
                })
            })
            .and_then(|v| {
                v.as_str().ok_or_else(|| {
                    simple_error!("Variable definition for {} ({v:?}) must be string", key)
                })
            })
            .map(Cow::from)
            .map_err(|e| e.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn creation_test() {
        let v: Value = serde_json::from_str(r#"{"try":"to"}"#).unwrap();
        let args = 
            TemplateArgs::new(v, Cursor::new(r#"Hello world, try ${try} keep up"#));
    }
    #[test]
    fn json_str() {
        let v = VariableMap::json_str(r#"{"try":"to"}"#).unwrap();
        let args =
            TemplateArgs::new(v, Cursor::new(r#"Hello world"#));
    }
}
