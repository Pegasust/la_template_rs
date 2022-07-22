mod replace_regex;
mod template_fs;
mod memfs_tracer;

use replace_regex::*;

use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    os::unix::prelude::FileExt,
    path::{PathBuf},
};

use enum_dispatch::enum_dispatch;
use itertools::{Itertools};
use la_template_base::{
    generate_template, parse_template, GenerateTemplate, VariableMap,
};
use common::{AnyErr, OptionVecTrait, MyResult, MyResultTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use simple_error::simple_error;

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ManagedVarSchema {
    #[serde(default = "HashMap::new")]
    metadata: HashMap<String, String>,
    var: PathBuf,
}

/// The main schema that we pass into the main function:
/// `la_template generate manager.json`
///
/// It should looks like; TODO: Complete this doc
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct ManagerSchema {
    vars: Vec<ManagedVarSchema>,
    templates: Vec<PathBuf>,
    replace_regex: Option<ReplaceRegexSchema>,
    skip_if_error: Option<bool>,
}

pub fn generate_with_handler(
    manager: ManagerSchema, fs: FileSystemImpl
) -> Result<(), Vec<AnyErr>> {
    // parse vars and templates separately
    let parsed_vars: Vec<MyResult<(_, VariableMap)>> = manager
        .vars
        .iter()
        .map(|v| {
            File::open(&v.var)
                .map_err(|e| e.into())
                .and_then(|f| serde_json::from_reader::<_, Value>(f).map_err(|e| e.into()))
                .map(|val| (&v.metadata, val.into()))
        })
        .collect::<Vec<_>>();
    let parsed_templates = manager
        .templates
        .iter()
        .map(|template_path| {
            File::open(template_path)
                .map_err(|e| e.into())
                .map(|template_f| BufReader::new(template_f))
                .and_then(|template_buf| {
                    parse_template(template_buf).map(|p| (template_path, p.into()))
                })
        })
        .collect::<Vec<_>>();
    let mut dispatched_regex = manager
        .replace_regex
        .unwrap_or_default()
        .compile()
        .map_err(|e| vec![e.into()])?;

    // now group errors aside from good ones
    let mut grouped_vars = parsed_vars
        .into_iter()
        .into_group_map_by(|r_mvar| matches!(r_mvar, Result::Ok(_)));
    let mut grouped_templates = parsed_templates
        .into_iter()
        .into_group_map_by(|r_temp| matches!(r_temp, Result::Ok(_)));

    let skip_error = manager.skip_if_error.unwrap_or(true);
    let err_msg = {
        // collect if either grouped_vars or grouped_templates have errs (false)
        let gv_err = grouped_vars.remove(&false).map(|v| {
            v.into_iter()
                .map(|r| r.result_str().unwrap_err())
                .join("\n")
        });
        let gt_err = grouped_templates.remove(&false).map(|v| {
            v.into_iter()
                .map(|r| r.result_str().unwrap_err())
                .join("\n")
        });
        [gv_err.unwrap_or_default(), gt_err.unwrap_or_default()].join("\n")
    };

    // Greedily show warnings or fail.
    if err_msg.len() > 0 {
        if !skip_error {
            return Err(vec![simple_error!(err_msg).into()]);
        }
        log::warn!("Failed to parse some template/variables:\n{err_msg}")
    }
    let clean_gv = grouped_vars
        .remove(&true)
        .to_vec()
        .into_iter()
        .map(|v| v.unwrap())
        // collect so that we own the data. Cartesian product only borrow
        .collect::<Vec<_>>();
    let clean_tm = grouped_templates
        .remove(&true)
        .to_vec()
        .into_iter()
        .map(|v| v.unwrap())
        .collect::<Vec<_>>();

    // Execute
    let err = clean_gv
        .iter()
        .cartesian_product(clean_tm.iter())
        .map(|((target, vars), (path, temp))| {
            dispatched_regex.dispatch(target)?;
            let location = dispatched_regex.regex_replace(path)?;
            // let location_f = File::create(location)?;
            GenerateTemplate {
                template: &temp,
                variables: &vars,
            }
            .generate()
            .and_then(
                |outp| generate_handler.write(location, outp), // location_f.write_all_at(outp.as_bytes(), 0u64)
                                                               //     .map_err(|e| e.into())
            )
        })
        .filter_map(|v| v.err())
        .collect::<Vec<_>>();
    if err.is_empty() {
        Ok(())
    } else {
        Err(err)
    }
}

pub fn generate(manager: ManagerSchema) -> Result<(), Vec<AnyErr>> {
    generate_with_handler(manager, Default::default())
}
