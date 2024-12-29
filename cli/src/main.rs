use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Result;
use argh::FromArgs;
use console::style;
use unicode_width::UnicodeWidthStr;

use fift::core::lexer::LexerPosition;
use fift::core::{Environment, SourceBlock};

use self::env::SystemEnvironment;
use self::input::LineReader;
use self::modules::*;
use self::util::{ArgsOrVersion, RestArgs, RestArgsDelimiter};

mod env;
mod input;
mod util;

mod modules;

/// A simple Fift interpreter. Type `bye` to quie,
/// or `words` to get a list of all commands
#[derive(FromArgs)]
struct App {
    /// do not preload standard preamble file `Fift.fif`
    #[argh(switch, short = 'n')]
    bare: bool,

    /// force interactive mode even if explicit source file names are indicated
    #[argh(switch, short = 'i')]
    interactive: bool,

    /// sets color-separated library source include path.
    /// If not indicated, $FIFTPATH is used instead
    #[argh(option, short = 'I')]
    include: Option<String>,

    /// sets an explicit path to the library source file.
    /// If not indicated, a default one will be used
    #[argh(option, short = 'L')]
    lib: Option<String>,

    /// a list of source files to execute (stdin will be used if empty)
    #[argh(positional)]
    source_files: Vec<String>,
}

#[allow(unused)]
#[derive(Default)]
struct ScriptModeDelim;

impl RestArgsDelimiter for ScriptModeDelim {
    const DELIM: &'static str = "-s";
    const DESCR: &'static str = r"script mode: use first argument as a fift source file and
                    import remaining arguments as $n";
}

fn main() -> Result<ExitCode> {
    let RestArgs(ArgsOrVersion::<App>(app), rest, ScriptModeDelim) = argh::from_env();

    // Prepare system environment
    let mut env = SystemEnvironment::with_include_dirs(
        &app.include
            .unwrap_or_else(|| std::env::var("FIFTPATH").unwrap_or_default()),
    );

    let interactive = app.interactive || rest.is_empty() && app.source_files.is_empty();

    // Prepare the source block which will be executed
    let mut stdout: Box<dyn std::io::Write> = Box::new(std::io::stdout());

    let mut source_blocks = Vec::new();

    if interactive {
        if std::io::stdin().is_terminal() {
            let mut line_reader = LineReader::new()?;
            stdout = line_reader.create_external_printer()?;
            source_blocks.push(SourceBlock::new("<stdin>", line_reader));
        } else {
            source_blocks.push(SourceBlock::new("<stdin>", std::io::stdin().lock()));
        }
    }

    if let Some(path) = rest.first() {
        source_blocks.push(env.include(path)?);
    }

    for path in app.source_files.into_iter().rev() {
        source_blocks.push(env.include(&path)?);
    }

    // Prepare preamble block
    if let Some(lib) = &app.lib {
        source_blocks.push(env.include(lib)?);
    } else if !app.bare {
        source_blocks.push(env.include(fift_libs::base_lib().name)?);
    }

    // Prepare Fift context
    let mut ctx = fift::Context::new(&mut env, &mut stdout)
        .with_basic_modules()?
        .with_module(CmdArgsUtils::new(rest))?
        .with_module(ShellUtils)?;

    for source_block in source_blocks {
        ctx.add_source_block(source_block);
    }

    // Execute
    loop {
        let error = match ctx.run() {
            Ok(exit_code) => return Ok(ExitCode::from(!exit_code)),
            Err(e) => e,
        };

        if interactive {
            eprintln!("{}", style("!!!").dim())
        }

        if let Some(pos) = ctx.input.get_position() {
            eprintln!("{}", Report { pos, error });
        };

        if let Some(next) = ctx.next.take() {
            eprintln!(
                "{}\n{}",
                style("backtrace:").red(),
                style(next.display_backtrace(&ctx.dicts.current)).dim()
            );
        }

        if !interactive {
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
        let word_start = std::cmp::min(self.pos.word_start, line.len());
        let word_end = std::cmp::min(self.pos.word_end, line.len());
        let (line_start, rest) = line.split_at(word_start);
        let (underlined, line_end) = rest.split_at(word_end - word_start);

        let line_start_len = UnicodeWidthStr::width(line_start);
        let underlined_len = UnicodeWidthStr::width(underlined);

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
