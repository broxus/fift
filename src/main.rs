use std::error::Error;
use std::io::{Read, Seek};
use std::path::Path;
use std::process::ExitCode;

struct SystemEnvironment;

impl fift::core::Environment for SystemEnvironment {
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
        Path::new(name).is_file()
    }

    fn write_file(&mut self, name: &str, contents: &[u8]) -> fift::Result<()> {
        std::fs::write(name, contents)?;
        Ok(())
    }

    fn read_file(&mut self, name: &str) -> fift::Result<Vec<u8>> {
        std::fs::read(name).map_err(From::from)
    }

    fn read_file_part(&mut self, name: &str, offset: u64, len: u64) -> fift::Result<Vec<u8>> {
        let mut result = Vec::new();

        let file = std::fs::File::open(name)?;
        let mut reader = std::io::BufReader::new(file);
        reader.seek(std::io::SeekFrom::Start(offset))?;
        reader.take(len).read_to_end(&mut result)?;
        Ok(result)
    }
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mut env = SystemEnvironment;
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout();

    let mut ctx = fift::Context::new(&mut env, &mut stdin, &mut stdout).with_basic_modules()?;

    let exit_code = ctx.run()?;

    Ok(ExitCode::from(!exit_code))
}
