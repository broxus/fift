use std::io::BufRead;

pub trait Environment {
    fn now_ms(&self) -> u64;

    fn get_env(&self, name: &str) -> Option<String>;

    fn file_exists(&self, name: &str) -> bool;

    fn write_file(&mut self, name: &str, contents: &[u8]) -> std::io::Result<()>;

    fn read_file(&mut self, name: &str) -> std::io::Result<Vec<u8>>;

    fn read_file_part(&mut self, name: &str, offset: u64, len: u64) -> std::io::Result<Vec<u8>>;

    fn include(&self, name: &str) -> std::io::Result<SourceBlock>;
}

pub struct SourceBlock {
    name: String,
    buffer: Box<dyn BufRead>,
}

impl SourceBlock {
    pub fn new<N: Into<String>, B: BufRead + 'static>(name: N, buffer: B) -> Self {
        Self {
            name: name.into(),
            buffer: Box::new(buffer),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn buffer_mut(&mut self) -> &mut dyn BufRead {
        &mut self.buffer
    }
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

    fn write_file(&mut self, _: &str, _: &[u8]) -> std::io::Result<()> {
        Ok(())
    }

    fn read_file(&mut self, name: &str) -> std::io::Result<Vec<u8>> {
        Err(not_found(name))
    }

    fn read_file_part(&mut self, name: &str, _: u64, _: u64) -> std::io::Result<Vec<u8>> {
        self.read_file(name)
    }

    fn include(&self, name: &str) -> std::io::Result<SourceBlock> {
        Err(not_found(name))
    }
}

fn not_found(name: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("`{name}` file not found"),
    )
}
