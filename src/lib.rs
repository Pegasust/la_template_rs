mod replace_regex;
mod template_fs;
mod memfs_tracer;

use replace_regex::*;
pub use template_fs::{
    FileImpl,
    FileStr,
    FileSystem,
    FileSystemImpl,
    FileTrait,
    MemFS,
    NaiveFS,
    OSFile,
    RootedFS
};
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
    generate_template, parse_template, AnyErr, GenerateTemplate, MyResult, MyResultTrait,
    OptionVecTrait, VariableMap,
};
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
/// It should looks like
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct ManagerSchema {
    vars: Vec<ManagedVarSchema>,
    templates: Vec<PathBuf>,
    replace_regex: Option<ReplaceRegexSchema>,
    skip_if_error: Option<bool>,
}

/// The trait specifying that this will handle the writing after
/// template generation is done
#[enum_dispatch]
pub trait WriteGeneratedOutput {
    fn write(&mut self, proposed_location: PathBuf, output: String) -> MyResult<()>;
}

#[enum_dispatch(WriteGeneratedOutput)]
pub enum WriteHandler {
    NaiveWriteToFile,
    RootedWriteToFile,
    WriteToSimpleVirtualFile
}

/// Context-free naively write to file without any consideration
/// of relative path, absolute path, or even the invoking location
/// 
/// This is best used for configurations that does only absolute path
/// 
/// TODO: Verify that everything is absolute path; warn on any
/// using relative path.
pub struct NaiveWriteToFile;
impl WriteGeneratedOutput for NaiveWriteToFile {
    fn write(&mut self, loc: PathBuf, outp: String) -> MyResult<()> {
        let location_f = File::create(loc)?;
        location_f
            .write_all_at(outp.as_bytes(), 0u64)
            .map_err(|e| e.into())
    }
}

/// TODO: Writes to the file, but will make every relative path points from
/// given root (from constructor)
pub struct RootedWriteToFile {
    root: PathBuf
}
impl WriteGeneratedOutput for RootedWriteToFile {
    fn write(&mut self,proposed_location:PathBuf,output:String) -> MyResult<()> {
        todo!()
    }
}

/// Writes to a simple virtual file system. This can be used to pipe onto
/// other processors before outputting because it retains the proposed
/// location along with the processed template.
/// 
/// Very helpful for testing and validation or even dry run.
/// 
/// TODO: Even better if we can attach logs or file-system operations
/// to this struct
pub struct WriteToSimpleVirtualFile {
    /// Stores a bucket-like file location
    bucket: HashMap<PathBuf, Vec<u8>>
}

impl Default for WriteToSimpleVirtualFile {
    fn default() -> Self {
        Self {bucket: Default::default()}
    }
}

impl WriteToSimpleVirtualFile {
    // some builder pattern

}

impl WriteGeneratedOutput for WriteToSimpleVirtualFile {
    fn write(&mut self,proposed_location:PathBuf,output:String) -> MyResult<()> {
        let overwrite_opt = self.bucket.get(&proposed_location).map(|last| last != output.as_bytes());
        let overwrite = matches!(overwrite_opt, Some(v) if v);
        if overwrite {
            return Err(simple_error!("Overwriting at {:?}", proposed_location).into())
        }
        self.bucket.insert(proposed_location, output.into_bytes());
        Ok(())
    }
}

pub fn generate_with_handler<W: WriteGeneratedOutput>(
    manager: ManagerSchema,
    mut generate_handler: W,
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
    generate_with_handler(manager, NaiveWriteToFile {})
}
