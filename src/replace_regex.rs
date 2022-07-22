use std::{collections::HashMap, path::{Path, PathBuf}, str::FromStr};

use common::MyResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use simple_error::simple_error;


#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub(crate) struct ReplaceRegexSchema {
    pattern: String,
    replace: String,
}

#[derive(Debug)]
pub(crate) struct ReplaceRegex {
    regex: Regex,
    replace: String,
    last_dispatch: Option<String>,
}

impl ReplaceRegex {
    pub(crate) fn dispatch(&mut self, target_metadata: &HashMap<String, String>) -> MyResult<&mut Self> {
        strfmt::strfmt(&self.replace, target_metadata)
            .map_err(|e| e.into())
            .map(|v| {
                self.last_dispatch = Some(v);
                self
            })
    }
    pub(crate) fn regex_replace<'a, P: AsRef<Path> + 'a>(&self, input: P) -> MyResult<PathBuf> {
        let repl = self
            .last_dispatch
            .as_ref()
            .ok_or_else(|| simple_error!("{:?} not dispatched", self))?;

        let path_str = input.as_ref().to_string_lossy();
        let replaced = self.regex.replace(&path_str, repl);
        PathBuf::from_str(&replaced).map_err(|e| e.into())
    }
}

impl ReplaceRegexSchema {
    pub(crate) fn compile(self) -> Result<ReplaceRegex, regex::Error> {
        Regex::new(self.pattern.as_ref()).map(|r| ReplaceRegex {
            regex: r,
            replace: self.replace,
            last_dispatch: None,
        })
    }
}

impl Default for ReplaceRegexSchema {
    fn default() -> Self {
        Self {
            pattern: "(.t)".to_string(),
            replace: "{target}".to_string(),
        }
    }
}
