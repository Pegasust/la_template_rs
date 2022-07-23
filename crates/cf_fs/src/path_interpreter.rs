use std::{borrow::Cow, collections::HashMap, path::{PathBuf, self}, str::{FromStr}};

use common::{res_ok, BytesToStringExt, MyResult, MyResultTrait};
use enum_dispatch::enum_dispatch;
use once_cell::unsync::Lazy;
use regex::bytes::Regex;
use simple_error::{simple_error};

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
#[derive(Debug, Clone)]
pub enum PathPlugin {
    ForwardPath,
    PathRemap,
    SuffixRelativePath,
}

impl AsRef<PathPlugin> for &PathPlugin {
    fn as_ref(&self) -> &PathPlugin {
        self
    }
}

impl Default for PathPlugin {
    fn default() -> Self {
        Self::ForwardPath(Default::default())
    }
}
#[derive(Debug, Clone)]
pub struct PathInterpreter {
    sequenced_plugin: Vec<PathPlugin>,
}

impl PathInterpreter {
    pub fn new<I, R>(iter: I) -> Self
    where
        I: Iterator<Item = R>,
        R: AsRef<PathPlugin>
    {
        Self {
            sequenced_plugin: iter
                .map(|plugin_ref| plugin_ref.as_ref().clone())
                .collect::<Vec<_>>(),
        }
    }
    pub fn then(mut self, p: PathPlugin) -> Self {
        self.sequenced_plugin.push(p);
        self
    }
    fn dedup<'a, 'b: 'a>(&self, input: Cow<'a, str>) -> MyResult<Cow<'b, str>> {
        self.sequenced_plugin
            .iter()
            .fold(Ok(input), |inp, plugin| {
                plugin.output(inp?)
            })
            // canonicalize
            .map(|s| {
                dedup_path_sep(s)
            })
    }
}

/// Singleton Interpreter
impl From<PathPlugin> for PathInterpreter {
    fn from(plugin: PathPlugin) -> Self {
        PathInterpreter::default().then(plugin)
    }
}

fn dedup_path_sep<'a, 'b: 'a>(path: Cow<'a, str>) -> Cow<'b, str> {
    PathBuf::from(path.as_ref())
        .iter().filter_map(|v| 
        if v.to_str().unwrap().contains(path::MAIN_SEPARATOR) {
            Some("")
        } else {
            v.to_str()
        })
        .collect::<Vec<_>>()
        .join("/")
        .into()
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use super::dedup_path_sep;

    fn dedup(input: &str) -> Cow<str> {
        dedup_path_sep(input.into())
    }
    #[test]
    fn forward() {
        assert_eq!(dedup("hello/world"), "hello/world");
        assert_eq!(dedup("hello"), "hello");
        assert_eq!(dedup("0/1/2/3/4"), "0/1/2/3/4");
    }
    #[test]
    fn filter() {
        assert_eq!(dedup("hello//world"), "hello/world");
        assert_eq!(dedup("hello/"), "hello");
        assert_eq!(dedup("0/1//2///3////4"), "0/1/2/3/4");
    }
}

impl PathPluggable for PathInterpreter {
    fn output<'a, S: AnyStr + 'a>(&self, input: S) -> MyResult<Cow<'a, str>> {
        self.dedup(Cow::from(input.as_ref()))
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
#[derive(Debug, Default, Clone, Copy)]
pub struct ForwardPath;
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
#[derive(Default, Debug, Clone)]
pub struct PathRemap {
    map: HashMap<MyString, MyString>,
}

impl PathRemap {
    pub fn new(map: HashMap<MyString, MyString>) -> Self {
        Self {map}
    }
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
                p_ref
            )
            .into())
        }
    }
    /// This func throws err in these cases:
    /// 1. Remap definition not given
    /// 2. Poorly formatted given input
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
        [prefix, suffix].concat().to_str().map(Cow::from)
    }
}
#[derive(Default, Debug, Clone)]
pub struct SuffixRelativePath {
    root: MyString,
}

impl SuffixRelativePath {
    pub fn new<S: AnyStr>(s: S) -> Self {
        let mut v = s.as_ref().as_bytes().to_vec();
        v.push(std::path::MAIN_SEPARATOR as u8);
        Self {
            root: v,
        }
    }
}

impl PathPluggableImpl for SuffixRelativePath {
    fn _validate<S: AnyStr>(&self, path: S) -> MyResult<S> {
        Ok(path)
    }

    fn _output<'a, S: AnyStr + 'a>(&self, validated_input: S) -> MyResult<Cow<'a, str>> {
        let input_ref = validated_input.as_ref();
        PathBuf::from_str(input_ref)
            .my_result()
            .and_then(|input_path| {
                if input_path.is_absolute() {
                    // pass through
                    Ok(input_ref.to_string().into())
                } else {
                    // process
                    [&self.root, input_ref.as_bytes()].concat()
                        .to_str()
                        .map(|v| v.into())
                }
            })
    }
}
