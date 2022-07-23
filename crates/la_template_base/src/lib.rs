// mod common;
use std::{io::{Seek, BufRead}, borrow::Cow, collections::HashMap};

use common::{bytes_to_string};
use common::{res_err, res_ok, MyResult, wrapper, wrap_fn};
use enum_dispatch::enum_dispatch;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use simple_error::simple_error;

pub struct GenerateTemplate<'a>
{
    pub template: &'a Template,
    pub variables: &'a VariableMap
}

impl <'t> GenerateTemplate<'t>
{
    /// Transforms all tokens to become [Cow<'_, str>]
    /// If there is something wrong before the apply process,
    /// it returns an Err
    /// 
    /// Otherwise, a [Iterator<Item=Result<str>>] (in the form of [Result::Ok])
    /// 
    /// Use [#Self::generate] for a more comprehensible result.
    pub fn dispatch(&self) -> MyResult<impl Iterator<Item=MyResult<Cow<'_, str>>>> {
        res_ok(self.validate_ref()?
            .apply())
    }
    pub fn generate(&self) -> MyResult<String> {
        let res = self.dispatch()?;
        let (sucs, errs): (Vec<_>, Vec<_>) = res.collect::<Vec<_>>()
            .into_iter()
            .partition_result();
        if !errs.is_empty() {
            res_ok(sucs.join(""))
        } else {
            res_err(errs.iter().map(|err|err.to_string()).join("\n"))
        }        
    }
    fn undefined_vars(&self) -> Vec<Cow<str>> {
        self.template.symbols().iter()
            .filter_map(|s| self.variables.get_defn(s).ok())
            .collect::<Vec<_>>()
    }
    fn validate_ref(&self) -> MyResult<&Self> {
        let undefined_vars = self.undefined_vars();
        if !undefined_vars.is_empty() {
            res_err(simple_error!("Missing definition: {:?}", undefined_vars))
        } else {
            res_ok(self)
        }
    }
    fn apply(&self) -> impl Iterator<Item=MyResult<Cow<'_, str>>> {
        self.template.tokens().iter()
            .map(|tok| match tok {
                Token::Str(s) => res_ok(Cow::from(s)),
                Token::Var(idx) => self.template.symbols()
                    .get(*idx as usize)
                    .ok_or_else(||simple_error!("Idx out of bounds: {}", idx).into())
                    .and_then(|var_name| {
                        self.variables.get_defn(var_name)
                    })
            })
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
struct TemplateParser<R> 
    where R: BufRead+Seek
{
    // on creation
    template: R,
    #[serde(default="sym")]
    sym: u8,
    #[serde(default="escape")]
    escape: u8,
    // on dispatch
    buf: Vec<u8>,
    tokens: Vec<Token>,
    symbs: Vec<String>,
}
const fn sym()->u8{b'$'}
const fn escape()->u8{b'\\'}

#[derive(Debug)]
enum SeekSymbol {
    EndOfFile,
    Symbol,
    Escape
}

impl <R> /*FnOnce()->MyResult<Template> for*/ TemplateParser<R>
    where R: BufRead+Seek
{
    fn call(mut self) -> MyResult<ConcreteTemplate> {
        loop {
            let (symb, token) = self.next_token()?;
            log::debug!("Next token: symb: {symb:?}, token: {token:?}");
            self.tokens.push(token);
            match symb {
                SeekSymbol::EndOfFile => break res_ok(()),
                SeekSymbol::Escape => continue,
                _ => {}
            }
            self.buf.clear();
            // we now hit the $ symbol, determine the var name
            let var_name = self.var_name()?;
            log::debug!("Var name: {var_name}");
            self.tokens.push(Token::Var(self.symbs.len() as u8));
            self.symbs.push(var_name);
        }?;
        res_ok(ConcreteTemplate {
            symbols: self.symbs,
            tokens: self.tokens
        })
    }
    pub fn new(template: R, sym: Option<u8>, escape: Option<u8>)->Self {
        Self { 
            template, 
            sym: sym.unwrap_or(b'$'), 
            escape: escape.unwrap_or(b'\\'), 
            buf: Default::default(), 
            tokens: Default::default(), 
            symbs: Default::default() 
        }
    }
    fn var_name(&mut self) -> MyResult<String> {
        unimplemented!("get varname after having found $ here");
    }
    fn next_token(&mut self) -> MyResult<(SeekSymbol, Token)> {
        self.template.read_until(self.sym, &mut self.buf)?;
        let symb_chr = self.buf.pop()
            // If it's not a self.sym, then we took the last chr of EOF, return it
            .and_then(|b| if b == self.sym {Some(b)} else {self.buf.push(b);None});
        log::debug!("Found {}u8; symb_chr: {symb_chr:?}", self.sym);
        if matches!(symb_chr, None) {
            // This will be the last self.token that is a literal.
            return res_ok((SeekSymbol::EndOfFile, Token::from_bytes(&self.buf)?));
        }
        let symb = symb_chr.unwrap();
        let chr_before = self.buf.pop();
        log::debug!("chr_before: {chr_before:?}");
        if let Some(b) = chr_before {
            if b == self.escape {
                self.buf.push(symb);
                return res_ok((SeekSymbol::Escape, Token::from_bytes(&self.buf)?));
            }
            self.buf.push(b);
        }
        res_ok((SeekSymbol::Symbol, Token::from_bytes(&self.buf)?))
    }
}

pub fn parse_template<R>(template: R)
    -> MyResult<ConcreteTemplate> 
    where R: BufRead + Seek 
{
    TemplateParser::new(template, None, None)
        .call()
}

pub fn generate_template<T, V>(template: T, variables: V) 
    -> MyResult<String> 
    where
        T: Into<Template>,
        V: Into<VariableMap>
{
    GenerateTemplate {template:&template.into(), variables:&variables.into()}.generate()
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Token {
    Str(String),
    Var(u8)
}
#[enum_dispatch]
pub trait TemplateTrait {
    fn tokens(&self) -> &Vec<Token>;
    // fn tokens_mut(&mut self) -> &mut Vec<Token>;
    fn symbols(&self) -> &Vec<String>;
    // fn symbols_mut(&mut self) -> &mut Vec<String>;
}

#[enum_dispatch(TemplateTrait)]
#[derive(Debug)]
pub enum Template {
    ConcreteTemplate,
    BufReadTemplate,
}

#[enum_dispatch]
pub trait VariableTrait {
    fn _get_defn<'a>(&'a self, key: &str) -> MyResult<Cow<'a, str>>;
    fn get_defn<AnyStr: AsRef<str>>(&self, key: AnyStr) -> MyResult<Cow<'_, str>> {
        self._get_defn(key.as_ref())
    }
}
#[enum_dispatch(VariableTrait)]
#[derive(Debug)]
pub enum VariableMap 
{
    HashMapStd(HashMap<String, String>),
    SerdeValue(Value)
}


// Implementations
#[derive(Debug)]
pub struct ConcreteTemplate {
    /// All of the tokens that makes up the template
    tokens: Vec<Token>,
    /// Contains the names of the variables declared in given template
    symbols: Vec<String>
}
impl TemplateTrait for ConcreteTemplate {
    fn tokens(&self) ->  &Vec<Token> {
        &self.tokens
    }

    // fn tokens_mut(&mut self) ->  &mut Vec<Token> {&mut self.tokens}

    fn symbols(&self) ->  &Vec<String> {
        &self.symbols
    }

    // fn symbols_mut(&mut self) ->  &mut Vec<String> {&mut self.symbols}
}

wrapper!(
#[derive(Debug)] 
pub BufReadTemplate wraps ConcreteTemplate
);

impl TemplateTrait for BufReadTemplate {
    wrap_fn!(fn tokens(&self) -> &Vec<Token>);
    wrap_fn!(fn symbols(&self) -> &Vec<String>);
    // wrap_fn!(fn tokens_mut(&mut self) -> &mut Vec<Token>);
    // wrap_fn!(fn symbols_mut(&mut self) -> &mut Vec<String>);
}

impl <R> From<R> for BufReadTemplate where R: BufRead+Seek {
    fn from(value: R) -> Self {
        Self::new(value).expect("Failed to parse given BufRead template")
    }
}

impl <R> From<R> for Template where R: BufRead+Seek {
    fn from(r: R) -> Self {
       BufReadTemplate::from(r).into() 
    }
}

impl BufReadTemplate {
    pub fn new<R>(read: R) -> MyResult<Self> where R: BufRead + Seek {
        Ok(Self(parse_template(read)?))
    }
}

impl <AnyStr> VariableTrait for HashMap<String, AnyStr> 
    where AnyStr: AsRef<str>
{
    fn _get_defn<'a>(& 'a self,key: &str) -> MyResult<Cow< 'a,str>> {
        self.get(key)
            .ok_or_else(||simple_error!("Var {} expected, but not defined").into())
            .map(|v| v.as_ref().into())
    }
}

impl <AnyStr> VariableTrait for HashMap<&str, AnyStr>
    where AnyStr: AsRef<str> 
{
    fn _get_defn<'a>(& 'a self,key: &str) -> MyResult<Cow<'a,str>> {
        self.get(key)
            .ok_or_else(||simple_error!("Var {} expected, but not defined").into())
            .map(|v| v.as_ref().into())
    }
}

impl VariableTrait for Value {
    fn _get_defn< 'a>(& 'a self,key: &str) -> MyResult<Cow< 'a,str>> {
        self.as_object().ok_or_else(||simple_error!("Given json is not str->str"))
            .and_then(|m| m.get(key).ok_or_else(||simple_error!("No such variable")))
            .and_then(|v| v.as_str().ok_or_else(||simple_error!("The mapping to value({:?}) is not str", v)))
            .map_err(|e|e.into())
            .map(|s| s.into())
    }
}

impl Token {
    pub fn from_bytes(bytes: &[u8]) -> MyResult<Self> {
        bytes_to_string(bytes).map(Token::Str)
    }
}

impl <AnyStr> From<AnyStr> for Token
    where AnyStr: AsRef<str>
{
    fn from(s: AnyStr) -> Self {
        Token::Str(s.as_ref().to_string())
    }
}
