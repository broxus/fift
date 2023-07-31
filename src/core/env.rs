use crate::error::*;

pub trait Environment {
    fn now_ms(&self) -> u64;

    fn get_env(&self, name: &str) -> Option<String>;

    fn file_exists(&self, name: &str) -> bool;

    fn write_file(&mut self, name: &str, contents: &[u8]) -> Result<()>;

    fn read_file(&mut self, name: &str) -> Result<Vec<u8>>;

    fn read_file_part(&mut self, name: &str, offset: u64, len: u64) -> Result<Vec<u8>>;
}

pub struct EmptyEnvironment;

impl Environment for EmptyEnvironment {
    fn now_ms(&self) -> u64 {
        0
    }

    fn get_env(&self, _: &str) -> Option<String> {
        None
    }

    fn file_exists(&self, _: &str) -> bool {
        false
    }

    fn write_file(&mut self, _: &str, _: &[u8]) -> Result<()> {
        Ok(())
    }

    fn read_file(&mut self, _: &str) -> Result<Vec<u8>> {
        Err(Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        )))
    }

    fn read_file_part(&mut self, name: &str, _: u64, _: u64) -> Result<Vec<u8>> {
        self.read_file(name)
    }
}
