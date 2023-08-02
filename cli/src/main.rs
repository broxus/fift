use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Result;
use argh::FromArgs;

use fift::core::{Environment, SourceBlock};

use self::env::SystemEnvironment;
use self::input::LineReader;
use self::util::ArgsOrVersion;

mod env;
mod input;
mod util;

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

fn main() -> Result<ExitCode> {
    let ArgsOrVersion::<App>(app) = argh::from_env();

    // Prepare system environment
    let mut env = SystemEnvironment::with_include_dirs(
        &app.include
            .unwrap_or_else(|| std::env::var("FIFTPATH").unwrap_or_default()),
    );

    // Prepare the source block which will be executed
    let mut stdout: Box<dyn std::io::Write> = Box::new(std::io::stdout());
    let base_source_block = if let Some(path) = app.source_file {
        env.include(&path)?
    } else if std::io::stdin().is_terminal() {
        let mut line_reader = LineReader::new()?;
        stdout = line_reader.create_external_printer()?;
        SourceBlock::new("<stdin>", line_reader)
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
    let mut ctx = fift::Context::new(&mut env, &mut stdout)
        .with_basic_modules()?
        .with_source_block(base_source_block);

    if let Some(lib) = library_source_block {
        ctx.add_source_block(lib);
    }

    // Execute
    match ctx.run() {
        Ok(exit_code) => Ok(ExitCode::from(!exit_code)),
        Err(e) => {
            use ariadne::{Color, Label, Report, ReportKind, Source};

            if let Some(next) = ctx.next {
                eprintln!("Backtrace:\n{}\n", next.display_backtrace(&ctx.dictionary));
            }

            let Some(pos) = ctx.input.get_position() else {
                return Err(e);
            };

            let id = pos.source_block_name;
            Report::build(ReportKind::Error, id, 0)
                .with_message(format!("{e:?}"))
                .with_label(
                    Label::new((id, pos.line_offset_start..pos.line_offset_end))
                        .with_color(Color::Red),
                )
                .finish()
                .eprint((id, Source::from(pos.line)))
                .unwrap();

            Ok(ExitCode::FAILURE)
        }
    }
}
