use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use argh::FromArgs;

use fift::core::{Environment, SourceBlock};

/// A simple Fift interpreter. Type `bye` to quie,
/// or `words` to get a list of all commands
#[derive(FromArgs)]
struct App {
    /// do not preload standard preamble file `Fift.fif`
    #[argh(switch, short = 'n')]
    bare: bool,

    /// sets color-separated library source include path.
    /// If not indicated, $FIFTPATH is used instead
    #[argh(option, short = 'I')]
    include: Option<String>,

    /// sets an explicit path to the library source file.
    /// If not indicated, a default one will be used
    #[argh(option, short = 'L')]
    lib: Option<String>,

    /// an optional path to the source file (stdin will be used otherwise)
    #[argh(positional)]
    source_file: Option<String>,
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let ArgsOrVersion::<App>(app) = argh::from_env();

    // Prepare system environment
    let mut env = SystemEnvironment {
        include_dirs: match app.include {
            Some(dirs) => split_dirs(&dirs),
            None => split_dirs(&std::env::var("FIFTPATH").unwrap_or_default()),
        },
    };

    // Prepare the source block which will be executed
    let base_source_block = if let Some(path) = app.source_file {
        env.include(&path)?
    } else {
        SourceBlock::new("<stdin>", std::io::stdin().lock())
    };

    // Prepare preamble block
    let library_source_block = if app.bare {
        None
    } else if let Some(lib) = &app.lib {
        Some(env.include(lib)?)
    } else {
        Some(SourceBlock::new(
            "<default Fift.fif>",
            std::io::Cursor::new(include_str!("Fift.fif")),
        ))
    };

    // Prepare Fift context
    let mut stdout = std::io::stdout();
    let mut ctx = fift::Context::new(&mut env, &mut stdout)
        .with_basic_modules()?
        .with_source_block(base_source_block);

    if let Some(lib) = library_source_block {
        ctx.add_source_block(lib);
    }

    // Execute
    let exit_code = ctx.run()?;
    Ok(ExitCode::from(!exit_code))
}

fn split_dirs(dirs: &str) -> Vec<PathBuf> {
    let dirs = dirs.trim();
    if dirs.is_empty() {
        return Vec::new();
    }
    dirs.split(',')
        .map(|item| PathBuf::from(item.trim()))
        .collect()
}

struct SystemEnvironment {
    include_dirs: Vec<PathBuf>,
}

impl SystemEnvironment {
    fn resolve_file(&self, name: &str) -> fift::Result<PathBuf> {
        if Path::new(name).is_file() {
            return Ok(PathBuf::from(name));
        }

        for dir in &self.include_dirs {
            let path = dir.join(name);
            if path.is_file() {
                return Ok(path);
            }
        }

        Err(fift::Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        )))
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

    fn write_file(&mut self, name: &str, contents: &[u8]) -> fift::Result<()> {
        std::fs::write(self.resolve_file(name)?, contents)?;
        Ok(())
    }

    fn read_file(&mut self, name: &str) -> fift::Result<Vec<u8>> {
        std::fs::read(self.resolve_file(name)?).map_err(From::from)
    }

    fn read_file_part(&mut self, name: &str, offset: u64, len: u64) -> fift::Result<Vec<u8>> {
        let mut result = Vec::new();

        let file = File::open(self.resolve_file(name)?)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(offset))?;
        reader.take(len).read_to_end(&mut result)?;
        Ok(result)
    }

    fn include(&self, name: &str) -> fift::Result<SourceBlock> {
        let file = File::open(self.resolve_file(name)?)?;
        let buffer = BufReader::new(file);
        Ok(fift::core::SourceBlock::new(name, buffer))
    }
}

// === CLI helpers ===

struct ArgsOrVersion<T: argh::FromArgs>(T);

impl<T: argh::FromArgs> argh::TopLevelCommand for ArgsOrVersion<T> {}

impl<T: argh::FromArgs> argh::FromArgs for ArgsOrVersion<T> {
    fn from_args(command_name: &[&str], args: &[&str]) -> Result<Self, argh::EarlyExit> {
        /// Also use argh for catching `--version`-only invocations
        #[derive(argh::FromArgs)]
        struct Version {
            /// print version information and exit
            #[argh(switch, short = 'v')]
            pub version: bool,
        }

        match Version::from_args(command_name, args) {
            Ok(v) if v.version => Err(argh::EarlyExit {
                output: format!("{} {}", command_name.first().unwrap_or(&""), VERSION),
                status: Ok(()),
            }),
            Err(exit) if exit.status.is_ok() => {
                let help = match T::from_args(command_name, &["--help"]) {
                    Ok(_) => unreachable!(),
                    Err(exit) => exit.output,
                };
                Err(argh::EarlyExit {
                    output: format!("{help}  -v, --version     print version information and exit"),
                    status: Ok(()),
                })
            }
            _ => T::from_args(command_name, args).map(|app| Self(app)),
        }
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
