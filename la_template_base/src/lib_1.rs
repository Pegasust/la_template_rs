mod common;
pub use common::{res_err, res_ok, AnyErr, MyResult, MyResultTrait};
use itertools::Itertools;
use serde_json::{Map, Value};
use simple_error::{require_with, simple_error};
use std::str;
use std::{
    borrow::Cow,
    io::{BufRead, Seek, SeekFrom},
};
use utf8_chars::BufReadCharsExt;

#[derive(Debug)]
enum Token {
    Str(String),
    Var(u8),
}

fn bytes_to_string(bytes: &[u8]) -> MyResult<String> {
    str::from_utf8(bytes).map(|s| s.to_string()).my_result()
}

impl Token {
    pub fn from_bytes(bytes: &[u8]) -> MyResult<Token> {
        bytes_to_string(bytes).map(Token::Str)
    }
}

impl<AnyStr> From<AnyStr> for Token
where
    AnyStr: AsRef<str>,
{
    fn from(str: AnyStr) -> Self {
        Token::Str(str.as_ref().to_string())
    }
}

#[derive(Default, Debug)]
pub struct Template {
    token: Vec<Token>,
}

/// Contains all of the variable names
#[derive(Default, Debug)]
pub struct Symbols(Vec<String>);
impl From<Symbols> for Vec<String> {
    fn from(s: Symbols) -> Self {
        s.0
    }
}

/// PREREQ: raw is pointing right after the '$' symbol.
///
/// A var name must be in form ${name even has space} -> "name even has space"
///
/// Note: one difficult part about non-encapsulated var name is that we need
/// to backtrack from the whitespace acting as a separator to preserve original string.
fn var_name<R: BufRead + Seek>(raw: &mut R) -> MyResult<String> {
    let c = require_with!(raw.read_char()?, "Unexpected EOF while parsing var");
    let mut buf = vec![0u8; 0];
    if c != '{' {
        return Err(simple_error!(
            "Unexpected char: {} at {}. \
            Var names not encapsulated are not (yet) supported. \
            Try: ${{my_name}} instead of $my_name.",
            c,
            raw.seek(SeekFrom::Current(0)).unwrap()
        )
        .into());
    }
    // the last read_char should have forwarded the reading cursor by 1 byte.
    // now we read until we see '}'
    match raw.read_until(b'}', &mut buf) {
        Ok(_) => {
            if buf[buf.len() - 1] != b'}' {
                return Err(simple_error!("Expecting a matching '}}'").into());
            }
            log::debug!("Found '}}': {}", bytes_to_string(&buf).unwrap());
            // found '}', it is a var name
            buf.pop();
            bytes_to_string(&buf)
        }
        Err(e) => res_err(e),
    }
}

fn parse_template<R: BufRead + Seek>(mut raw: R) -> MyResult<(Template, Symbols)> {
    let mut template = Template::default();
    let mut symbs = Symbols::default();

    let mut buf: Vec<u8> = Vec::new();
    loop {
        const SYM: u8 = b'$';
        const ESCAPE: u8 = b'\\';
        match raw.read_until(SYM, &mut buf) {
            Ok(_) => {
                let chr_symb_opt = buf.pop().and_then(|b| {
                    if b == SYM {
                        Some(b)
                    } else {
                        buf.push(b);
                        None
                    }
                });
                log::debug!("Found \'{SYM}\'; chr_symb_opt: {chr_symb_opt:?}");
                if matches!(chr_symb_opt, None) {
                    log::debug!("EOF");
                    template.token.push(Token::from_bytes(&buf)?);
                    break Ok(());
                }
                let chr_before = buf.pop();
                log::debug!("chr_before: {chr_before:?}");
                match chr_before {
                    Some(b) => {
                        if b == ESCAPE {
                            buf.push(chr_symb_opt.unwrap());
                            log::debug!("\"{}\" is escape!", bytes_to_string(&buf).unwrap());
                            continue;
                        } else {
                            buf.push(b);
                            log::debug!("\"{}\" to be parsed", bytes_to_string(&buf).unwrap());
                        }
                    }
                    None => {
                        log::debug!("First substitution hit");
                    } // first var substitution
                }
                // it is var, we need to add the buffer so far
                // as a Token::Str
                template.token.push(Token::from_bytes(&buf)?);
                buf.clear();
                // now parse the var name
                let var_name = var_name(&mut raw)?;
                log::debug!("Var name: {var_name}");
                template.token.push(Token::Var(symbs.0.len() as u8));
                symbs.0.push(var_name);
            }
            Err(e) => break res_err(e),
        };
    }?;
    res_ok((template, symbs))
}

fn validate<'a>(
    s: &'a Symbols,
    var_map: &'a Map<String, Value>,
) -> MyResult<(&'a Symbols, &'a Map<String, Value>)> {
    // Find all of the variables that are missing definition
    let undefined_vars = get_undefined_vars(s, var_map);
    if !undefined_vars.is_empty() {
        res_err(simple_error!(
            "Missing definition of [{}]",
            undefined_vars.iter().join(",")
        ))
    } else {
        res_ok((s, var_map))
    }
}

fn get_undefined_vars<'a>(s: &'a Symbols, var_map: &'a Map<String, Value>) -> Vec<&'a String> {
    s.0.iter()
        .filter(|&e| !var_map.contains_key(e))
        .collect::<Vec<_>>()
}

pub enum Warning<T> {
    Ok(T),
    Partial(T, AnyErr),
}

impl<T> Warning<T> {
    pub fn from<E: Into<AnyErr>>(partial: T, might_err: Option<E>) -> Self {
        match might_err {
            Some(err) => Warning::Partial(partial, err.into()),
            None => Warning::Ok(partial),
        }
    }
}

fn apply_u(
    temp: Template,
    symb: &Symbols,
    vars_as_map: &Map<String, Value>,
) -> MyResult<Warning<String>> {
    // pipeline: Token::Var -> String
    let mut err: Option<String> = None;
    let output = temp
        .token
        .iter()
        .map_while(|tok| match tok {
            Token::Str(s) => Some(Cow::from(s)),
            Token::Var(idx) => symb
                .0
                .get(*idx as usize)
                .ok_or(simple_error!("Index out of bound: {idx}"))
                .and_then(|name| {
                    vars_as_map
                        .get(name)
                        .ok_or(simple_error!("name {name} not defined in given variables"))
                })
                .and_then(|v| {
                    v.as_str()
                        .ok_or(simple_error!("value {v:?} is not string."))
                })
                .map(|s| s.into())
                .map_err(|e| err = Some(e.to_string()))
                .ok(),
        })
        .join("");
    Ok(Warning::from(output, err))
}
fn apply(
    (temp, symb, vars_as_map): (Template, &Symbols, &Map<String, Value>),
) -> MyResult<Warning<String>> {
    apply_u(temp, symb, vars_as_map)
}

fn generate_template_partial<R: BufRead + Seek>(
    template: R,
    variables: Value,
) -> MyResult<Warning<String>> {
    let vars_as_map = variables
        .as_object()
        .ok_or(simple_error!("Given variables do not form a map"))?;
    let (template, symbols) = parse_template(template)?;
    log::debug!("template: {template:?}; Symbols: {symbols:?}");
    validate(&symbols, vars_as_map)
        .map(|(s, v)| (template, s, v))
        .and_then(apply)
}

pub fn generate_template<R: BufRead + Seek>(template: R, variables: Value) -> MyResult<String> {
    generate_template_partial(template, variables).and_then(|warn| match warn {
        Warning::Ok(s) => Ok(s),
        Warning::Partial(s, err) => Err(simple_error!("Failed: {:?};\nPartial:{}", s, err).into()),
    })
}
