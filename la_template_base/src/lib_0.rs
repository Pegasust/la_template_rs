mod common;
use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek},
};

use common::MyResult;
use enum_dispatch::enum_dispatch;
use serde_json::Value;
use simple_error::simple_error;

// Schemas

struct TemplateArgs {
    variable: VariableMap,
    template: Template,
}

#[enum_dispatch(BufRead)]
pub enum Template {
    TemplateStr,
    TemplateFile,
}
pub struct TemplateStr(Cursor<String>);
impl Read for TemplateStr {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}
impl BufRead for TemplateStr {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
    }
}
pub struct TemplateFile(BufReader<File>);
impl Read for TemplateFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl BufRead for TemplateFile {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
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

impl VariableMap {
    pub fn json_str<AnyStr: AsRef<str>>(s: AnyStr) -> MyResult<VariableMap> {
        let v: Value = serde_json::from_str(s.as_ref())?;
        Ok(v.into())
    }
}
impl ReadVariable for HashMap<String, String> {
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
    #[test]
    fn create_template_args() {
        let v = TemplateArgs {
            template: TemplateStr(Cursor::new("hello".to_string())).into(),
            variable: HashMap::from([
                ("hello".to_string(), "world".to_string()),
                ("this".to_string(), "that".to_string()),
            ])
            .into(),
        };
    }
}
