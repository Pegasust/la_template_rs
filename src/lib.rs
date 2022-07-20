use std::{path::{PathBuf, Path}, borrow::Cow, fs::File, io::BufReader, str::FromStr, os::unix::prelude::FileExt};

use itertools::{Product, iproduct, Itertools};
use la_template_base::{MyResult, parse_template, MyResultTrait, generate_template};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use simple_error::simple_error;

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ManagedVarSchema {
    target: String,
    var: PathBuf,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ReplaceRegexSchema {
    pattern: String,
    replace: String
}

impl ReplaceRegexSchema {
    fn dispatch(&mut self)->MyResult<&mut Self> {
        unimplemented!("dispatch promised variables into self.replace");
        Ok(self)
    }
    fn regex_replace<'a, P: AsRef<Path>+'a>(&self, input: P)->MyResult<PathBuf> {
        // lazy_static!{static ref regex: Regex = Regex::new(self.pattern.as_ref()).unwrap();}
        let regex = Regex::new(self.pattern.as_ref())?;
        // convert path to str, then back
        let path_str = input.as_ref().to_string_lossy();
        let replaced = regex.replace(&path_str, &self.replace);
        PathBuf::from_str(&replaced)
            .map_err(|e| e.into())
    }
}

impl Default for ReplaceRegexSchema {
    fn default() -> Self {
        Self { pattern: "(.t)".to_string(), replace: "{target}".to_string() }
    }
}

/// The main schema that we pass into the main function:
/// `la_template generate manager.json`
/// 
/// It should looks like
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ManagerSchema {
    vars: Vec<ManagedVarSchema>,
    templates: Vec<PathBuf>,
    replace_regex: Option<ReplaceRegexSchema>,
    skip_if_error: Option<bool>
}

pub fn generate(mut manager: ManagerSchema) -> MyResult<String> {
    // parse vars and templates separately
    let parsed_vars: Vec<MyResult<(_,Value)>> = manager.vars.iter()
        .map(|v| {
            File::open(&v.var).map_err(|e| e.into())
                .and_then(|f| serde_json::from_reader(f).map_err(|e| e.into()))
                .map(|val| (&v.target, val))
        })
        .collect::<Vec<_>>();
    let parsed_templates = manager.templates.iter()
        .map(|template_path| {
            File::open(template_path).map_err(|e|e.into())
                .map(|template_f| BufReader::new(template_f))
                .and_then(|template_buf| 
                    parse_template(template_buf).map(|p|(template_path, p))
                )
        })
        .collect::<Vec<_>>();
    let mut dispatched_regex = manager.replace_regex.unwrap_or_default();
    dispatched_regex.dispatch()?;

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

    // Handle parsing errors
    if err_msg.len() > 0 {
        if !skip_error {return Err(simple_error!(err_msg).into())}
        log::warn!("Failed to parse some template/variables:\n{err_msg}")
    }
    let clean_gv = grouped_vars.get(&true);
    let clean_tm = grouped_templates.get(&true);
    if clean_gv.is_none() || clean_tm.is_none() {
        // Nothing to build
        return Ok(Default::default())
    }
    
    // Execute
    clean_gv.unwrap().into_iter().cartesian_product(clean_tm.unwrap().into_iter())
        .map(|(vars, temp)| (&vars.unwrap(), &temp.unwrap()))
        .map(|((target, vars), (path, temp))| {
            let location = dispatched_regex.regex_replace(path)?;
            let location_f = File::create(location)?;
            generate_template(temp, vars)
                .and_then(|outp| 
                    location_f.write_all_at(outp.as_bytes(), 0u64)
                        .map_err(|e| e.into())
                )
        });

    Ok(Default::default())
}
