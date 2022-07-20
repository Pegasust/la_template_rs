use std::{path::{PathBuf, Path}, borrow::Cow, fs::File, io::BufReader, str::FromStr, os::unix::prelude::FileExt, collections::HashMap};

use itertools::{Product, iproduct, Itertools};
use la_template_base::{MyResult, parse_template, MyResultTrait, generate_template, GenerateTemplate, VariableMap, OptionVecTrait, AnyErr};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use simple_error::simple_error;



#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ReplaceRegexSchema {
    pattern: String,
    replace: String
}

#[derive(Debug)]
struct ReplaceRegex {
    regex: Regex,
    replace: String,
    last_dispatch: Option<String>
}

impl ReplaceRegex {
    fn dispatch(&mut self, target_metadata: &HashMap<String, String>) -> MyResult<&mut Self> {
        strfmt::strfmt(&self.replace, target_metadata)
            .map_err(|e| e.into())
            .map(|v| {self.last_dispatch = Some(v); self})
    }
    fn regex_replace<'a, P: AsRef<Path>+'a>(&self, input: P) -> MyResult<PathBuf> {
        let repl = self.last_dispatch.as_ref().ok_or_else(||simple_error!("{:?} not dispatched", self))?;

        let path_str = input.as_ref().to_string_lossy();
        let replaced = self.regex.replace(&path_str, repl);
        PathBuf::from_str(&replaced)
            .map_err(|e| e.into())
    }
}

impl ReplaceRegexSchema {
    fn compile(self) -> Result<ReplaceRegex, regex::Error> {
        Regex::new(self.pattern.as_ref()).map(|r| 
            ReplaceRegex{ regex: r, replace: self.replace, last_dispatch: None })
    }
}

impl Default for ReplaceRegexSchema {
    fn default() -> Self {
        Self { pattern: "(.t)".to_string(), replace: "{target}".to_string() }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ManagedVarSchema {
    #[serde(default="HashMap::new")]
    metadata: HashMap<String, String>,
    var: PathBuf,
}


/// The main schema that we pass into the main function:
/// `la_template generate manager.json`
/// 
/// It should looks like
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct ManagerSchema {
    vars: Vec<ManagedVarSchema>,
    templates: Vec<PathBuf>,
    replace_regex: Option<ReplaceRegexSchema>,
    skip_if_error: Option<bool>
}

pub fn generate(manager: ManagerSchema) -> Result<(), Vec<AnyErr>> {
    // parse vars and templates separately
    let parsed_vars: Vec<MyResult<(_,VariableMap)>> = manager.vars.iter()
        .map(|v| {
            File::open(&v.var).map_err(|e| e.into())
                .and_then(|f| serde_json::from_reader::<_, Value>(f).map_err(|e| e.into()))
                .map(|val| (&v.metadata, val.into()))
        })
        .collect::<Vec<_>>();
    let parsed_templates = manager.templates.iter()
        .map(|template_path| {
            File::open(template_path).map_err(|e|e.into())
                .map(|template_f| BufReader::new(template_f))
                .and_then(|template_buf| 
                    parse_template(template_buf).map(|p|(template_path, p.into()))
                )
        })
        .collect::<Vec<_>>();
    let mut dispatched_regex = manager.replace_regex.unwrap_or_default().compile()
        .map_err(|e| vec![e.into()])?;

    // now group errors aside from good ones
    let mut grouped_vars = parsed_vars.into_iter()
        .into_group_map_by(|r_mvar| matches!(r_mvar, Result::Ok(_)));
    let mut grouped_templates = parsed_templates.into_iter()
        .into_group_map_by(|r_temp| matches!(r_temp, Result::Ok(_)));

    let skip_error = manager.skip_if_error.unwrap_or(true);
    let err_msg = {
        // collect if either grouped_vars or grouped_templates have errs (false)
        let gv_err = grouped_vars
            .remove(&false)
            .map(|v|{
                v.into_iter().map(|r| r.result_str().unwrap_err())
                    .join("\n")
            });
        let gt_err = grouped_templates.remove(&false)
            .map(|v| {
                v.into_iter().map(|r| r.result_str().unwrap_err())
                    .join("\n")
            });
        [gv_err.unwrap_or_default(), gt_err.unwrap_or_default()].join("\n")
    };

    // Greedily show warnings or fail.
    if err_msg.len() > 0 {
        if !skip_error {return Err(vec![simple_error!(err_msg).into()])}
        log::warn!("Failed to parse some template/variables:\n{err_msg}")
    }
    let clean_gv = grouped_vars
        .remove(&true)
        .to_vec()
        .into_iter().map(|v| v.unwrap())
        // collect so that we own the data. Cartesian product only borrow
        .collect::<Vec<_>>();
    let clean_tm = grouped_templates
        .remove(&true)
        .to_vec()
        .into_iter().map(|v| v.unwrap())
        .collect::<Vec<_>>();
    
    // Execute
    let err = clean_gv.iter().cartesian_product(clean_tm.iter())
        .map(|((target, vars), (path, temp))| {
            dispatched_regex.dispatch(target)?;
            let location = dispatched_regex.regex_replace(path)?;
            let location_f = File::create(location)?;
            GenerateTemplate{template: &temp, variables: &vars}.generate()
                .and_then(|outp| 
                    location_f.write_all_at(outp.as_bytes(), 0u64)
                        .map_err(|e| e.into())
                )
        })
        .filter_map(|v| v.err())
        .collect::<Vec<_>>();
    
    if err.is_empty() {Ok(())} else {Err(err)}
}
