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

    fn resolve_file(&self, name: &str) -> Result<PathBuf> {
        if Path::new(name).is_file() {
            return Ok(PathBuf::from(name));
        }

        for dir in &self.include_dirs {
            let path = dir.join(name);
            if path.is_file() {
                return Ok(path);
            }
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
            .as_micros() as u64
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
        std::fs::read(self.resolve_file(name)?).map_err(From::from)
    }

    fn read_file_part(&mut self, name: &str, offset: u64, len: u64) -> std::io::Result<Vec<u8>> {
        let mut result = Vec::new();

        let file = File::open(self.resolve_file(name)?)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(offset))?;
        reader.take(len).read_to_end(&mut result)?;
        Ok(result)
    }

    fn include(&self, name: &str) -> std::io::Result<SourceBlock> {
        let file = File::open(self.resolve_file(name)?)?;
        let buffer = BufReader::new(file);
        Ok(fift::core::SourceBlock::new(name, buffer))
    }
}
