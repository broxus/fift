use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Result;
use argh::FromArgs;
use console::style;

use fift::core::lexer::LexerPosition;
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

    let is_interactive = std::io::stdin().is_terminal();

    // Prepare the source block which will be executed
    let mut stdout: Box<dyn std::io::Write> = Box::new(std::io::stdout());
    let base_source_block = if let Some(path) = app.source_file {
        env.include(&path)?
    } else if is_interactive {
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
    loop {
        let error = match ctx.run() {
            Ok(exit_code) => return Ok(ExitCode::from(!exit_code)),
            Err(e) => e,
        };

        if is_interactive {
            eprintln!("{}", style("!!!").dim())
        }

        if let Some(pos) = ctx.input.get_position() {
            eprintln!("{}", Report { pos, error });
        };

        if let Some(next) = ctx.next.take() {
            eprintln!(
                "{}\n{}",
                style("backtrace:").red(),
                style(next.display_backtrace(&ctx.dictionary)).dim()
            );
        }

        if !is_interactive {
            return Ok(ExitCode::FAILURE);
        }

        eprintln!();
        ctx.input.reset_until_base();
        ctx.stack.clear();
    }
}

struct Report<'a, E> {
    pos: LexerPosition<'a>,
    error: E,
}

impl<E> std::fmt::Display for Report<'_, E>
where
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line_number = self.pos.line_number.to_string();
        let offset_len = line_number.len();
        let offset = format!("{:offset_len$}", "");

        let arrow = style("-->").blue().bold();
        let block = style("|").blue().bold();
        let line_number = style(line_number).blue().bold();

        let line = self.pos.line.trim_end();
        let (line_start, rest) = line.split_at(self.pos.word_start);
        let (underlined, line_end) = rest.split_at(self.pos.word_end - self.pos.word_start);

        let line_start_len = line_start.len();
        let underlined_len = underlined.len();

        write!(
            f,
            "{}{:?}\n\
            {offset}{arrow} {}:{}:{}\n\
            {offset} {block}\n\
            {line_number} {block} {}{}{}\n\
            {offset} {block} {:line_start_len$}{}\n\
            {offset} {block}",
            style("error: ").red(),
            style(&self.error).bold(),
            self.pos.source_block_name,
            self.pos.line_number,
            self.pos.word_start + 1,
            line_start,
            style(underlined).red(),
            line_end,
            "",
            style(format!("{:->1$}", "", underlined_len)).red(),
        )
    }
}
