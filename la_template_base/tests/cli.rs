use la_template_base::*;
use serde_json::Value;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::Once;

fn wrapped<AnyStr0: AsRef<str>, AnyStr1: AsRef<str>>(
    template_file: AnyStr0,
    var_file: AnyStr1,
) -> MyResult<String> {
    let template_buf = BufReader::new(File::open(template_file.as_ref())?);
    let vars: Value = serde_json::from_reader(BufReader::new(File::open(var_file.as_ref())?))?;
    generate_template(template_buf, vars)
}

fn setup() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {});
    let _ = env_logger::builder().is_test(true).try_init();
}

#[test]
fn basic_example() {
    setup();
    assert_eq!(
        wrapped("tests/hello_report.t.txt", "tests/hello_report.json")
            .expect("Should generate concrete output"),
        "hello world, this is pegasust reporting. The total cost is $12."
    );
}

#[test]
fn first_sub() {
    setup();
    assert_eq!(
        wrapped("tests/first_sub.t.txt", "tests/first_sub.json")
            .expect("Should generate concrete output"),
        "The moon is beautiful isn't it, Naomi?"
    )
}

fn str_input<AnyStr0: AsRef<str>, AnyStr1: AsRef<str>>(
    template: AnyStr0,
    vars: AnyStr1,
) -> MyResult<String> {
    generate_template(
        Cursor::new(template.as_ref()),
        serde_json::from_str::<Value>(vars.as_ref())?,
    )
}

#[test]
fn from_string() {
    setup();
    let string_rb = "We can also construct string templates${too}";
    assert_eq!(
        str_input(string_rb, r#"{"too":"t_o_o"}"#).expect("Should generate concrete output"),
        "We can also construct string templatest_o_o"
    );
}

#[test]
fn missing_var_def() {
    setup();
    let template = r#"Many ${var} is ${status}"#;
    let vars = r#"{
        "var": "defined"
    }"#;
    let err = str_input(template, vars).expect_err("Should say missing vars");
    println!("err: {err:?}");

    let err = str_input(template, "{}").expect_err("Missing vars");
    println!("err: {err:?}");

    let spare_and_missing = str_input(template,r#"{"var":"def", "other": "d", "somethingelse":"unrelated"}"#)
        .expect_err("Missing vars");
    println!("spare_and_missing: {spare_and_missing:?}");

    let spare =
        str_input(template, r#"{"var":"def","status":"good","over":"defined"}"#)
            .expect("Over-defining is non-error");
    assert_eq!(spare, "Many def is good".to_string());
}
