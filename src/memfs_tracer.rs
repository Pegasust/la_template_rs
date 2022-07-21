use std::path::Path;

use enum_dispatch::enum_dispatch;

#[enum_dispatch]
pub trait Trace {
    fn on_open<P>(&mut self, path: P) where P: AsRef<Path>;
    fn on_open_nonexist<P>(&mut self, path: P) where P: AsRef<Path>;

    fn on_create<P>(&mut self, path: P) where P: AsRef<Path>;
    fn on_create_overwrite<P>(&mut self, path: P, last_content: Option<&Vec<u8>>) where P: AsRef<Path>;
    
    fn on_write_overwrite<P>(&mut self, path: P, last_content: Option<&Vec<u8>>) where P: AsRef<Path> {
        self.on_create_overwrite(path, last_content)
    }
}
#[enum_dispatch(Trace)]
pub enum Tracer {
    NoopTracer
}

impl Default for Tracer {
    fn default() -> Self {
        Self::NoopTracer(Default::default())
    }
}

pub struct NoopTracer;
impl Default for NoopTracer {
    fn default() -> Self {
        Self { }
    }
}
impl Trace for NoopTracer {
    fn on_open<P>(&mut self,_path:P)where P:AsRef<Path> {
        
    }

    fn on_open_nonexist<P>(&mut self,_path:P)where P:AsRef<Path> {
        
    }

    fn on_create<P>(&mut self,_path:P)where P:AsRef<Path> {
        
    }

    fn on_create_overwrite<P>(&mut self,_path:P, _last_content: Option<&Vec<u8>>)where P:AsRef<Path> {
        
    }
}
