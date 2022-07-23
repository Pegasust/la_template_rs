use std::{path::{Path, PathBuf}, collections::HashMap, io::{Read, Write, BufReader, BufWriter, Seek, SeekFrom}};

use common::{MyResult, MyResultTrait, AnyErr, wrap_fn};
use enum_dispatch::enum_dispatch;
use simple_error::simple_error;

use crate::memfs_tracer::{Tracer, Trace};

#[derive(Default)]
pub struct FileSystem {
    fs_impl: FileSystemImpl
}

impl <'a> FileSystem {
    pub fn open<P>(&'a mut self, path: P) -> MyResult<File<'a>> where P: AsRef<Path> {
        self.fs_impl.open(path).map(|f| File{ f_impl: f })
    }
    pub fn create<P>(&'a mut self, path: P) -> MyResult<File<'a>> where P: AsRef<Path> {
        self.fs_impl.create(path).map(|f| File{ f_impl: f })
    }
    pub fn bufread<P>(&'a mut self, path: P) -> MyResult<BufReader<File>> where P: AsRef<Path> {
        self.open(path).map(std::io::BufReader::new)
    }
    pub fn bufwrite<P>(&'a mut self, path: P) -> MyResult<BufWriter<File>> where P: AsRef<Path> {
        self.create(path).map(std::io::BufWriter::new)
    }
}

#[derive(Debug)]
pub struct File<'a> {
    f_impl: FileImpl<'a>
}
impl <'a> File<'a> {
    #[inline]
    fn get_mut(&mut self) -> &mut FileImpl<'a> {
        &mut self.f_impl
    }
}

impl <'a> Read for File<'a> {
    wrap_fn!(fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>);
}
impl <'a> Write for File<'a> {
    wrap_fn!(fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>);
    wrap_fn!(fn flush(&mut self) -> std::io::Result<()>);
}
impl <'a> Seek for File<'a> {
    wrap_fn!(fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64>);
}

#[enum_dispatch(ProvideFileSystem)]
enum FileSystemImpl {
    OSFileSystem,
    MemFileSystem
}

impl Default for FileSystemImpl {
    fn default() -> Self {
        OSFileSystem::default().into()
    }
}

impl From<OSFileSystem> for FileSystemImpl {
    fn from(s: OSFileSystem) -> Self {
        Self::OSFileSystem(s)
    }
}

impl From<MemFileSystem> for FileSystemImpl {
    fn from(s: MemFileSystem) -> Self {
        Self::MemFileSystem(s)
    }
}

impl <'a> ProvideFileSystem<'a> for FileSystemImpl {
    fn open<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path> {
        match self {
            Self::OSFileSystem(fs) => fs.open(path),
            Self::MemFileSystem(mfs) => mfs.open(path)
        }
    }

    fn create<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path> {
        match self {
            Self::OSFileSystem(fs) => fs.create(path),
            Self::MemFileSystem(mfs) => mfs.create(path)
        }
    }
}

trait ProvideFileSystem<'a> where Self: 'a {
    // NOTE: open is mut because it may write to an attached tracer

    fn open<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path>;
    fn create<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path>;
}
#[derive(Debug)]
enum FileImpl<'a> {
    OSFile(std::fs::File),
    MemFile(MemFile<'a>)
}
impl <'a> Read for FileImpl<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::OSFile(f) => f.read(buf),
            Self::MemFile(mf) => mf.read(buf)
        }
    }
}
impl <'a> Write for FileImpl<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::OSFile(f) => f.write(buf),
            Self::MemFile(mf) => mf.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::OSFile(f) => f.flush(),
            Self::MemFile(mf) => mf.flush()
        }
    }
}

impl <'a> Seek for FileImpl<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::OSFile(f) => f.seek(pos),
            Self::MemFile(mf) => {
                let offset = match pos {
                    SeekFrom::Start(n) => {
                        mf.offset = n as usize;
                        return Ok(n);
                    },
                    SeekFrom::End(n) => (mf.f_impl.vec().len(), n),
                    SeekFrom::Current(n) => (mf.offset, n)
                };
                (offset.0 as i64).checked_add(offset.1)
                    .and_then(|v| {
                        if v < 0 {
                            None
                        } else {
                            mf.offset = v as usize;
                            Some(v as u64)
                        }})
                    .ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidInput, 
                        simple_error!("Bad seek: Invalid or overflow position")))
            }
        }
    }
}

impl <'a> From<MemFile<'a>> for FileImpl<'a> {
    fn from(v: MemFile<'a>) -> Self {
        Self::MemFile(v)
    }
}
impl <'a> TryInto<MemFile<'a>> for FileImpl<'a> {
    type Error = AnyErr;

    fn try_into(self) -> Result<MemFile<'a>, Self::Error> {
        match self {
            FileImpl::MemFile(v) => Ok(v),
            _ => Err(simple_error!("FileImpl is not FileImpl::MemFile").into())
        }
    }
}
impl <'a> MemFile<'a> {
    fn read_f(content: &'a Vec<u8>)->Self {
        Self { f_impl: MemFileImpl::MemFileRead(content), offset:0 }
    }
    fn write_f(content: &'a mut Vec<u8>) -> Self {
        Self { f_impl: MemFileImpl::MemFileWrite(content), offset:0 }
    }
}

impl <'a> From<std::fs::File> for FileImpl<'a> {
    fn from(f: std::fs::File) -> Self {
        Self::OSFile(f)
    }
}
impl <'a> TryInto<std::fs::File> for FileImpl<'a> {
    type Error=AnyErr;
    fn try_into(self) -> Result<std::fs::File, Self::Error> {
        match self {
            FileImpl::OSFile(f) => Ok(f),
            _ => Err(simple_error!("Not an OSFile").into())
        }
    }
}
#[derive(Debug)]
enum MemFileImpl<'a> {
    MemFileRead(&'a Vec<u8>),
    MemFileWrite(&'a mut Vec<u8>)
}

impl <'a> MemFileImpl<'a> {
    fn vec(&self) -> &Vec<u8> {
        match self {
            Self::MemFileRead(v) => v,
            Self::MemFileWrite(v) => v
        }
    }
    fn vec_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Self::MemFileRead(_) => None,
            Self::MemFileWrite(v) => Some(v)
        }
    }
}

#[derive(Debug)]
struct MemFile<'a> {
    f_impl: MemFileImpl<'a>,
    offset: usize
    // descriptor: PathBuf
}

impl <'a> MemFile<'a> {
    fn remain(&self) -> &[u8] {
        let len = self.offset.min(self.f_impl.vec().len());
        &self.f_impl.vec()[len..]
    }
}

struct MemFileSystem {
    bucket: HashMap<PathBuf, Vec<u8>>,
    fs_tracer: Tracer
}

impl <'a> ProvideFileSystem<'a> for MemFileSystem where Self: 'a {
    fn open<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path> {
        self.bucket.get(path.as_ref())
            .ok_or_else(||{
                self.fs_tracer.on_open_nonexist(path.as_ref());
                simple_error!("Path {:?} not found in provided MemFileSystem", path.as_ref())
            })
            .my_result()
            .map(|content_ref| MemFile::read_f(content_ref).into())
    }

    fn create<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path> {
        let p_ref = path.as_ref();
        if let Some(last) = self.bucket.insert(p_ref.to_path_buf(), vec![0u8;0]) {
            self.fs_tracer.on_create_overwrite(path.as_ref(), Some(&last))
        }
        Ok(self.bucket.get_mut(p_ref)
            .map(|v| MemFile::write_f(v).into())
            .expect("HashMap: insert, but cannot get_mut right after"))
            
    }
}

#[derive(Default)]
struct OSFileSystem;

impl <'a> ProvideFileSystem<'a> for OSFileSystem {
    fn open<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path> {
        std::fs::File::open(path).my_result().map(|v| v.into())
    }

    fn create<P>(&'a mut self, path: P) -> MyResult<FileImpl<'a>> where P: AsRef<Path> {
        std::fs::File::create(path).my_result().map(|v| v.into())
    }
}

impl <'a> Read for MemFile<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = Read::read(&mut self.remain(), buf)?;
        self.offset += read;
        Ok(read)
    }
}

impl <'a> Write for MemFile<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let vec = self.f_impl.vec_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Unsupported,
                simple_error!("File does not support writing")
            )
        })?;
        vec.write_all(buf).map(|_| buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}