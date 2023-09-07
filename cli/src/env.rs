use std::fs::File;
use std::io::{BufReader, Read, Result, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use fift::core::{Environment, SourceBlock};

pub struct SystemEnvironment {
    include_dirs: Vec<PathBuf>,
}

impl SystemEnvironment {
    pub fn with_include_dirs(dirs: &str) -> Self {
        let dirs = dirs.trim();
        let include_dirs = if dirs.is_empty() {
            Vec::new()
        } else {
            dirs.split(':')
                .map(|item| PathBuf::from(item.trim()))
                .collect()
        };
        Self { include_dirs }
    }

    fn resolve_file(&self, name: &str) -> Result<Resolved> {
        if Path::new(name).is_file() {
            return Ok(Resolved::File(PathBuf::from(name)));
        }

        for dir in &self.include_dirs {
            let path = dir.join(name);
            if path.is_file() {
                return Ok(Resolved::File(path));
            }
        }

        if let Some(lib) = fift_libs::all().get(name) {
            return Ok(Resolved::Lib(*lib));
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("`{name}` file not found"),
        ))
    }
}

impl Environment for SystemEnvironment {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn get_env(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn file_exists(&self, name: &str) -> bool {
        self.resolve_file(name).is_ok()
    }

    fn write_file(&mut self, name: &str, contents: &[u8]) -> std::io::Result<()> {
        std::fs::write(name, contents)?;
        Ok(())
    }

    fn read_file(&mut self, name: &str) -> std::io::Result<Vec<u8>> {
        match self.resolve_file(name)? {
            Resolved::File(path) => std::fs::read(path),
            Resolved::Lib(lib) => Ok(lib.as_bytes().to_vec()),
        }
    }

    fn read_file_part(&mut self, name: &str, offset: u64, len: u64) -> std::io::Result<Vec<u8>> {
        fn read_part<R>(mut r: R, offset: u64, len: u64) -> std::io::Result<Vec<u8>>
        where
            R: Read + Seek,
        {
            let mut result = Vec::new();
            r.seek(SeekFrom::Start(offset))?;
            r.take(len).read_to_end(&mut result)?;
            Ok(result)
        }

        match self.resolve_file(name)? {
            Resolved::File(path) => {
                let r = BufReader::new(File::open(path)?);
                read_part(r, offset, len)
            }
            Resolved::Lib(lib) => read_part(std::io::Cursor::new(lib), offset, len),
        }
    }

    fn include(&self, name: &str) -> std::io::Result<SourceBlock> {
        Ok(match self.resolve_file(name)? {
            Resolved::File(path) => {
                let file = File::open(path)?;
                let buffer = BufReader::new(file);
                fift::core::SourceBlock::new(name, buffer)
            }
            Resolved::Lib(lib) => fift::core::SourceBlock::new(name, std::io::Cursor::new(lib)),
        })
    }
}

enum Resolved {
    File(PathBuf),
    Lib(&'static str),
}
