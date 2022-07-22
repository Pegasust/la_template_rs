use std::{borrow::Cow, collections::HashMap, io::Read, path::PathBuf, str::FromStr};

use common::{res_ok, BytesToStringExt, MyResult, MyResultTrait};
use enum_dispatch::enum_dispatch;
use once_cell::unsync::Lazy;
use regex::bytes::Regex;
use simple_error::{simple_error, SimpleError};

type MyString = Vec<u8>;
pub trait AnyStr: AsRef<str> {}
impl<T: AsRef<str>> AnyStr for T {}

#[enum_dispatch]
trait PathPluggableImpl {
    fn _validate<S: AnyStr>(&self, path: S) -> MyResult<S>;
    fn _output<'a, S: AnyStr + 'a>(&self, validated_input: S) -> MyResult<Cow<'a, str>>;
}

#[enum_dispatch]
pub trait PathPluggable {
    fn output<'a, S: AnyStr + 'a>(&self, input: S) -> MyResult<Cow<'a, str>>;
}
impl<T: PathPluggableImpl> PathPluggable for T {
    fn output<'a, S: AnyStr + 'a>(&self, input: S) -> MyResult<Cow<'a, str>> {
        self._validate(input).and_then(|s| self._output(s))
    }
}

#[enum_dispatch(PathPluggableImpl)]
#[derive(Debug)]
pub enum PathPlugin {
    ForwardPath,
    PathRemap,
    SuffixRelativePath,
}

impl Default for PathPlugin {
    fn default() -> Self {
        Self::ForwardPath(Default::default())
    }
}

pub struct PathInterpreter {
    sequenced_plugin: Vec<PathPlugin>,
}

impl PathInterpreter {
    pub fn new<I>(iter: I) -> Self
    where
        I: Iterator<Item = PathPlugin>,
    {
        Self {
            sequenced_plugin: iter.collect::<Vec<_>>(),
        }
    }
    pub fn then(mut self, p: PathPlugin) -> Self {
        self.sequenced_plugin.push(p);
        self
    }
}

impl PathPluggable for PathInterpreter {
    fn output<'a, S: AnyStr + 'a>(&self, input: S) -> MyResult<Cow<'a, str>> {
        self.sequenced_plugin
            .iter()
            .fold(Ok(Cow::from(input.as_ref().to_string())), |inp, plugin| {
                plugin.output(inp?)
            })
    }
}

impl Default for PathInterpreter {
    fn default() -> Self {
        Self {
            sequenced_plugin: vec![Default::default()],
        }
    }
}

// implementation
#[derive(Debug)]
pub struct ForwardPath;
impl Default for ForwardPath {
    fn default() -> Self {
        ForwardPath {}
    }
}
impl PathPluggableImpl for ForwardPath {
    fn _validate<S: AnyStr>(&self, path: S) -> MyResult<S> {
        Ok(path)
    }
    fn _output<'a, S>(&self, validated_input: S) -> MyResult<Cow<'a, str>>
    where
        S: AnyStr + 'a,
    {
        Ok(Cow::from(validated_input.as_ref().to_string()))
    }
}
#[derive(Debug)]
pub struct PathRemap {
    map: HashMap<MyString, MyString>,
}

impl PathRemap {
    fn regex(&self) -> Lazy<Regex> {
        Lazy::new(|| Regex::new(r"^(@(\w+))?(/?[\w/._ -]*)$").unwrap())
    }
}

impl PathPluggableImpl for PathRemap {
    fn _validate<S: AnyStr>(&self, path: S) -> MyResult<S> {
        let p_ref = path.as_ref();
        if self.regex().is_match(p_ref.as_bytes()) {
            Ok(path)
        } else {
            Err(simple_error!(
                r#"Path input "{}" does not match remapable pattern"#,
                p_ref.clone()
            )
            .into())
        }
    }

    fn _output<'a, S: AnyStr + 'a>(&self, validated_input: S) -> MyResult<Cow<'a, str>> {
        let matches = self
            .regex()
            .captures(validated_input.as_ref().as_bytes())
            .unwrap();
        let dummy_empty = vec![0u8; 0];
        let prefix = matches.get(2).map_or_else(
            || res_ok(&dummy_empty),
            |remap| {
                let var = remap.as_bytes();
                self.map.get(var).ok_or_else(|| {
                    simple_error!(
                        r"Given path remap doesn't have var {:?}",
                        common::bytes_to_string(var)
                    )
                    .into()
                })
            },
        )?;
        let suffix = matches.get(3).unwrap().as_bytes();
        [prefix, suffix].concat().to_str().map(|v| Cow::from(v))
    }
}
#[derive(Debug)]
pub struct SuffixRelativePath {
    root: MyString,
}

impl SuffixRelativePath {
    pub fn new<S: AnyStr>(s: S) -> Self {
        Self {
            root: s.as_ref().as_bytes().to_vec(),
        }
    }
}

impl PathPluggableImpl for SuffixRelativePath {
    fn _validate<S: AnyStr>(&self, path: S) -> MyResult<S> {
        Ok(path)
    }

    fn _output<'a, S: AnyStr + 'a>(&self, validated_input: S) -> MyResult<Cow<'a, str>> {
        let input_ref = validated_input.as_ref();
        PathBuf::from_str(input_ref.clone())
            .my_result()
            .and_then(|input_path| {
                input_path
                    .is_absolute()
                    .then_some(input_ref.clone().to_string().into())
                    .ok_or(())
                    .or_else(|_| {
                        [&self.root, input_ref.clone().as_bytes()]
                            .concat()
                            .to_str()
                            .map(|v| Cow::from(v))
                    })
            })
    }
}
