//! Filesystem facade that this crates uses

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::prelude::FileExt;
use std::path::PathBuf;
use std::{path::Path, io::BufReader};
use std::fs::File;
use enum_dispatch::enum_dispatch;
use la_template_base::{MyResult, wrapper};
use simple_error::simple_error;

use crate::memfs_tracer::{Tracer, Trace};
#[enum_dispatch]
pub trait FileSystem<'a> {
    fn open<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>>
        where P: AsRef<Path>;
    fn create<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>>
        where P: AsRef<Path>;
}
// #[enum_dispatch(FileSystem<'a>)]
pub enum FileSystemImpl {
    /// No concerns about difference in program invocation and the relative
    /// link from current file
    NaiveFS(NaiveFS),
    /// Flexible in changing the "root" directory. We can change the root
    /// as we go.
    /// 
    /// It is preferred to give it an absolute path, although
    /// relative path does work just as well
    /// 
    /// TODO: Consider path mapping like:
    /// ../../tests/config.js, given a path mapping file/db
    /// @tests -> ../../tests
    /// we can then write @tests/config.js
    RootedFS(RootedFS),
    /// A quasi-virtual filesystem that stores everything on memory
    /// (using a Map/HashMap).
    /// 
    /// This has some use cases: 
    /// 
    /// - Testing without overriding the file structure
    /// - Pass the filesystem onto the next process
    /// 
    MemFS(MemFS)
}

#[enum_dispatch]
pub trait FileTrait {
    // TODO: make these into references if possible
    fn read_all(&mut self) -> MyResult<Vec<u8>>;
    fn write_all(&mut self, content: Vec<u8>) -> MyResult<()>;
}
#[enum_dispatch(FileTrait)]
pub enum FileImpl<'a> {
    OSFile,
    FileStr(FileStr<'a>)
}


pub struct NaiveFS;
impl Default for NaiveFS {
    fn default() -> Self {
        Self {}
    }
}
impl NaiveFS {
    fn adapt<'a>(f: Result<File, std::io::Error>) -> MyResult<FileImpl<'a>> {
        f.map_err(|e| e.into())
         .map(|v| OSFile(v).into())
    }
}
impl <'a> FileSystem<'a> for NaiveFS {
    fn open<P>(&mut self,path:P) -> MyResult<FileImpl<'a>>where P:AsRef<Path> {
        Self::adapt(File::open(path))
    }

    fn create<P>(&mut self,path:P) -> MyResult<FileImpl<'a>>where P:AsRef<Path> {
        Self::adapt(File::create(path))
    }
}

wrapper!(#[derive(Debug)] pub OSFile wraps File);

#[enum_dispatch(FileTrait)]
pub enum FileStr<'a> {
    FileReadStr(FileReadStr<'a>),
    FileWriteStr(FileWriteStr<'a>)
}

impl <'a> FileStr<'a> {
    pub fn read<P>(fs: &'a MemFS, key: P) -> Self
        where P: AsRef<Path>
    {
        FileReadStr::new(fs, key).into()
    }
    pub fn write<P>(fs: &'a mut MemFS, key: P) -> Self 
        where P: AsRef<Path>
    {
        FileWriteStr::new(fs, key).into()
    }
}

pub struct FileReadStr<'a> {
    fs: &'a MemFS,
    key: PathBuf
}
pub struct FileWriteStr<'a> {
    fs: &'a mut MemFS,
    key: PathBuf
}


impl <'a> FileWriteStr<'a> {
    pub fn new<P>(fs: &'a mut MemFS, key: P) -> Self
        where P: AsRef<Path> 
    {
        Self { fs: fs, key: key.as_ref().to_path_buf() }
    }
}

impl <'a> FileReadStr<'a> {
    pub fn new<P>(fs: &'a MemFS, key: P) -> Self
        where P: AsRef<Path>
    {
        Self{fs: fs, key: key.as_ref().to_path_buf()}
    }
}

impl FileTrait for OSFile {
    fn read_all(&mut self) -> MyResult<Vec<u8>> {
        let mut buf = Vec::<u8>::new();
        self.0.read(&mut buf).map(|_s| buf)
            .map_err(|e| e.into())
    }

    fn write_all(&mut self, content:Vec<u8>) -> MyResult<()> {
        self.0.write_all_at(&content, 0u64)
            .map_err(|e| e.into())
    }
}

impl <'a> FileTrait for FileWriteStr<'a> {
    fn read_all(&mut self) -> MyResult<Vec<u8>> {
        self.fs.bucket.get(&self.key)
            .ok_or_else(|| simple_error!("Attempt to read at non existent file {:?}", self.key).into())
            .map(|v| v.clone())
    }

    fn write_all(&mut self,content:Vec<u8>) -> MyResult<()> {
        let err: Option<MyResult<()>> = self.fs.bucket.insert(self.key.clone(), content)
            .map(|last_v| self.fs.fs_tracer.on_write_overwrite(self.key.clone(), Some(&last_v)))
            .map(|_| Ok(()));
        match err {
            None => Ok(()),
            Some(res) => res.map_err(|e| e.into())
        }
    }
}

impl <'a> FileTrait for FileReadStr<'a> {
    fn read_all(&mut self) -> MyResult<Vec<u8>> {
        self.fs.bucket.get(&self.key)
            .ok_or_else(|| simple_error!("Attempt to read at non existent file {:?}", self.key).into())
            .map(|v| v.clone())
    }

    fn write_all(&mut self,_content:Vec<u8>) -> MyResult<()> {
        Err(simple_error!("Attempting to write to FileReadStr").into())
    }
}

pub struct RootedFS {
    nfs: NaiveFS,
    root: PathBuf
}

impl RootedFS {
    pub fn new<P: AsRef<Path>>(p: P) -> Self {
        Self { nfs: Default::default(), root: p.as_ref().to_path_buf() }
    }
    pub fn change_root<P: AsRef<Path>>(&mut self, new_root: P) -> &mut Self {
        self.root = new_root.as_ref().to_path_buf();
        self
    }

    fn rooted_path_of<P: AsRef<Path>>(&self, path:P) -> PathBuf {
        let p = path.as_ref();
        if p.is_absolute() {
            p.to_owned()
        } else {
            self.root.join(p)
        }
    }
}

impl Default for RootedFS {
    fn default() -> Self {
        Self::new::<PathBuf>(Default::default())
    }
}

impl <'a> FileSystem<'a> for RootedFS {
    fn open<P>(&mut self,path:P) -> MyResult<FileImpl>where P:AsRef<Path> {
        self.nfs.open(self.rooted_path_of(path))
    }

    fn create<P>(&mut self,path:P) -> MyResult<FileImpl>where P:AsRef<Path> {
        self.nfs.create(self.rooted_path_of(path))
    }
}

pub struct MemFS {
    bucket: HashMap<PathBuf, Vec<u8>>,
    fs_tracer: Tracer
}

impl MemFS {
    pub fn path_iter(&self) -> std::collections::hash_map::Keys<'_, PathBuf, Vec<u8>>{
        self.bucket.keys()
    }
}

impl Default for MemFS {
    fn default() -> Self {
        Self { bucket: Default::default(), fs_tracer: Default::default() }
    }
}

impl <'a> FileSystem<'a> for MemFS {
    fn open<P>(&'a mut self,path:P) -> MyResult<FileImpl<'a>>where P:AsRef<Path> {
        let p_ref = path.as_ref();
        self.fs_tracer.on_open(p_ref.clone());
        self.bucket.get(p_ref.clone())
            .ok_or_else(|| {
                self.fs_tracer.on_open_nonexist(p_ref.clone());
                simple_error!("File {:?} doesn't exist", p_ref.clone()).into()
            })
            .map(|_v| FileStr::read(self, p_ref.clone()).into())
    }

    fn create<P>(&'a mut self,path:P) -> MyResult<FileImpl>where P:AsRef<Path> {
        let p_ref = path.as_ref();
        self.fs_tracer.on_create(p_ref.clone());
        self.bucket.insert(p_ref.clone().to_path_buf(), Vec::new())
            .map(|old_v| self.fs_tracer.on_create_overwrite(p_ref.clone(), Some(&old_v)));
        Ok(FileStr::write(self, p_ref.clone()).into())
    }
}

impl <'a> FileSystem<'a> for FileSystemImpl {
    fn open<P>(& 'a mut self,path:P) -> MyResult<FileImpl< 'a>>where P:AsRef<Path> {
        match self {
            Self::MemFS(mem) => mem.open(path),
            Self::NaiveFS(nfs) => nfs.open(path),
            Self::RootedFS(rfs) => rfs.open(path)
        }
    }

    fn create<P>(& 'a mut self,path:P) -> MyResult<FileImpl< 'a>>where P:AsRef<Path> {
        match self {
            Self::MemFS(mem) => mem.create(path),
            Self::NaiveFS(nfs) => nfs.create(path),
            Self::RootedFS(rfs) => rfs.create(path)
        }
    }
}

impl From<MemFS> for FileSystemImpl {
    fn from(s: MemFS) -> Self {
        FileSystemImpl::MemFS(s)
    }
}

impl From<NaiveFS> for FileSystemImpl {
    fn from(s: NaiveFS) -> Self {
        FileSystemImpl::NaiveFS(s)
    }
}
impl From<RootedFS> for FileSystemImpl {
    fn from(s: RootedFS) -> Self {
        FileSystemImpl::RootedFS(s)
    }
}


#[cfg(test)]
mod test {
    use std::str::FromStr;

    use itertools::Itertools;

    use super::*;
    fn debug_memfs() -> MemFS {
        Default::default()
    }
    fn pathbuf<AnyStr: AsRef<str>>(s: AnyStr) -> PathBuf {
        PathBuf::from_str(s.as_ref()).unwrap()
    }
    fn validate_read_on_write<V>(f: &mut FileImpl, validate_if_can_read: V) -> MyResult<Option<Vec<u8>>>
        where V: Fn(&Vec<u8>) -> bool
    {
        match f.read_all() {
            // it's ok to throw err if we read in a file declared to write
            Err(_) => {Ok(None)},
            // if we may read a file declared to write, this file should be
            // empty because it is not yet existed
            Ok(v) => {
                if validate_if_can_read(&v) {
                    Ok(Some(v))
                } else {
                    Err(simple_error!("Can read, but validation on {:?} failed", v).into())
                }
            }
        }
    }
    #[test]
    fn memfs_empty_init() {
        let mfs = debug_memfs();
        let paths = mfs.path_iter().collect_vec();
        assert_eq!(paths.len(), 0);
    }
    #[test]
    fn memfs_put_once() {
        let mut mfs = debug_memfs();
        let mut f = mfs.create("random_path").expect("Fail to create new file from empty memfs");
        validate_read_on_write(&mut f, |v| v.is_empty())
            .expect("Reading on write bad behavior");
        f.write_all("hello world".as_bytes().to_vec()).expect("Fail to write to MemFS::create file");
        let paths = mfs.path_iter().collect_vec();
        assert_eq!(paths, vec![&pathbuf("random_path")]);
        // now test that the content is expected
        let content = mfs.open("random_path").expect("Fail to open existing file")
            .read_all().expect("Fail to read opened existing file");
        assert_eq!(std::str::from_utf8(&content).unwrap(), "hello world")            
    }
}