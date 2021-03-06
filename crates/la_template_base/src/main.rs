use clap::Parser;
use la_template_base::{generate_template};
use common::AnyErr;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about=None)]
struct Args {
    /// The path to the template file.
    /// The template file should looks like this:
    /// ```
    /// hello ${world_name}, this is ${name} reporting. This is escaped \$11.00.
    /// ```
    #[clap(short, long, value_parser)]
    template: PathBuf,
    /// The path to a JSON file that lists at least all
    /// of the variables declared in template.
    #[clap(short, long, value_parser)]
    var_json: PathBuf,
}

fn main_result() -> Result<(), AnyErr> {
    let args = Args::parse();
    let template_f = File::open(args.template)?;
    let var_f = File::open(args.var_json)?;
    let template_bf = BufReader::new(template_f);
    let vars: Value = serde_json::from_reader(BufReader::new(var_f))?;
    let output = generate_template(template_bf, vars)?;
    std::io::stdout()
        .write_all(output.as_bytes())
        .map_err(|err| err.into())
}

fn main() {
    main_result()
        .unwrap_or_else(|err| eprintln!("Failed to parse given templates: {}", err))
}
