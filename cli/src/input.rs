use std::io::{BufRead, Write};

use anyhow::Result;
use rustyline::{DefaultEditor, ExternalPrinter};

pub struct LineReader {
    editor: DefaultEditor,
    line: String,
    offset: usize,
    finished: bool,
}

impl LineReader {
    pub fn new() -> Result<Self> {
        let editor = DefaultEditor::new()?;
        Ok(Self {
            editor,
            line: String::default(),
            offset: 0,
            finished: false,
        })
    }

    pub fn create_external_printer(&mut self) -> Result<Box<dyn Write>> {
        let printer = self.editor.create_external_printer()?;
        Ok(Box::new(TerminalWriter(printer)))
    }
}

struct TerminalWriter<T>(T);

impl<T: ExternalPrinter> Write for TerminalWriter<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .print(String::from_utf8_lossy(buf).into_owned())
            .expect("External print failure");
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::io::Read for LineReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        let n = {
            let mut rem = self.fill_buf()?;
            rem.read(buf)?
        };
        self.consume(n);
        Ok(n)
    }
}

impl std::io::BufRead for LineReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        use rustyline::error::ReadlineError;

        if self.offset >= self.line.len() {
            loop {
                match self.editor.readline("> ") {
                    Ok(line) if line.is_empty() => continue,
                    Ok(mut line) => {
                        line.push('\n');
                        self.line = line;
                        self.offset = 0;
                        break;
                    }
                    Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                        self.line = Default::default();
                        self.offset = 0;
                        self.finished = true;
                        break;
                    }
                    Err(ReadlineError::Io(e)) => return Err(e),
                    Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
                }
            }
        }
        Ok(self.line[self.offset..].as_bytes())
    }

    fn consume(&mut self, amt: usize) {
        self.offset += amt;
    }
}
