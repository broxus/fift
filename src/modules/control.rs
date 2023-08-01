use std::rc::Rc;

use anyhow::{Context as _, Result};

use crate::core::*;
use crate::error::{ExecutionAborted, UnexpectedEof};

pub struct Control;

#[fift_module]
impl Control {
    #[init]
    fn init(d: &mut Dictionary) -> Result<()> {
        d.define_word(
            "'exit-interpret ",
            DictionaryEntry {
                definition: Rc::new(ExitInterpretCont),
                active: false,
            },
            false,
        )
    }

    // === Execution control ===

    #[cmd(name = "execute", tail)]
    fn interpret_execute(ctx: &mut Context) -> Result<Option<Cont>> {
        let cont = ctx.stack.pop_cont()?;
        Ok(Some(*cont))
    }

    #[cmd(name = "times", tail)]
    fn interpret_execute_times(ctx: &mut Context) -> Result<Option<Cont>> {
        let count = ctx.stack.pop_smallint_range(0, 1000000000)? as usize;
        let body = ctx.stack.pop_cont()?;
        Ok(match count {
            0 => None,
            1 => Some(*body),
            _ => {
                ctx.next = Some(Rc::new(cont::TimesCont {
                    body: Some(Rc::clone(&body)),
                    after: ctx.next.take(),
                    count: count - 1,
                }));
                Some(*body)
            }
        })
    }

    #[cmd(name = "if", tail)]
    fn interpret_if(ctx: &mut Context) -> Result<Option<Cont>> {
        let true_ref = ctx.stack.pop_cont()?;
        Ok(if ctx.stack.pop_bool()? {
            Some(*true_ref)
        } else {
            None
        })
    }

    #[cmd(name = "ifnot", tail)]
    fn interpret_ifnot(ctx: &mut Context) -> Result<Option<Cont>> {
        let false_ref = ctx.stack.pop_cont()?;
        Ok(if ctx.stack.pop_bool()? {
            None
        } else {
            Some(*false_ref)
        })
    }

    #[cmd(name = "cond", tail)]
    fn interpret_cond(ctx: &mut Context) -> Result<Option<Cont>> {
        let false_ref = ctx.stack.pop_cont()?;
        let true_ref = ctx.stack.pop_cont()?;
        Ok(Some(if ctx.stack.pop_bool()? {
            *true_ref
        } else {
            *false_ref
        }))
    }

    #[cmd(name = "while", tail)]
    fn interpret_while(ctx: &mut Context) -> Result<Option<Cont>> {
        let body = ctx.stack.pop_cont()?;
        let cond = ctx.stack.pop_cont()?;
        ctx.next = Some(Rc::new(cont::WhileCont {
            condition: Some(Rc::clone(&cond)),
            body: Some(*body),
            after: ctx.next.take(),
            running_body: true,
        }));
        Ok(Some(*cond))
    }

    #[cmd(name = "until", tail)]
    fn interpret_until(ctx: &mut Context) -> Result<Option<Cont>> {
        let body = ctx.stack.pop_cont()?;
        ctx.next = Some(Rc::new(cont::UntilCont {
            body: Some(Rc::clone(&body)),
            after: ctx.next.take(),
        }));
        Ok(Some(*body))
    }

    // === Compiler control ===

    #[cmd(name = "[", active)]
    fn interpret_internal_interpret_begin(ctx: &mut Context) -> Result<()> {
        ctx.state.begin_interpret_internal()?;
        ctx.stack.push_argcount(0, ctx.dictionary.make_nop())
    }

    #[cmd(name = "]", active)]
    fn interpret_internal_interpret_end(ctx: &mut Context) -> Result<()> {
        ctx.state.end_interpret_internal()?;
        ctx.stack.push(ctx.dictionary.make_nop())
    }

    #[cmd(name = "{", active)]
    fn interpret_wordlist_begin(ctx: &mut Context) -> Result<()> {
        ctx.state.begin_compile()?;
        interpret_wordlist_begin_aux(&mut ctx.stack)?;
        ctx.stack.push_argcount(0, ctx.dictionary.make_nop())
    }

    #[cmd(name = "}", active)]
    fn interpret_wordlist_end(ctx: &mut Context) -> Result<()> {
        ctx.state.end_compile()?;
        interpret_wordlist_end_aux(ctx)?;
        ctx.stack.push_argcount(1, ctx.dictionary.make_nop())
    }

    #[cmd(name = "({)", stack)]
    fn interpret_wordlist_begin_aux(stack: &mut Stack) -> Result<()> {
        stack.push(WordList::default())
    }

    #[cmd(name = "(})")]
    fn interpret_wordlist_end_aux(ctx: &mut Context) -> Result<()> {
        let word_list = ctx.stack.pop_word_list()?;
        ctx.stack.push(word_list.finish())
    }

    #[cmd(name = "(compile)")]
    fn interpret_compile_internal(ctx: &mut Context) -> Result<()> {
        ctx.compile_stack_top()
    }

    #[cmd(name = "(execute)", tail)]
    fn interpret_execute_internal(ctx: &mut Context) -> Result<Option<Cont>> {
        let cont = ctx.execute_stack_top()?;
        Ok(Some(cont))
    }

    #[cmd(name = "'", active)]
    fn interpret_tick(ctx: &mut Context) -> Result<()> {
        let word = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;
        let entry = match ctx.dictionary.lookup(word.data) {
            Some(entry) => entry,
            None => ctx
                .dictionary
                .lookup(&format!("{} ", word.data))
                .with_context(|| format!("Undefined word `{}`", word.data))?,
        };
        ctx.stack.push(entry.definition.clone())?;
        ctx.stack.push_argcount(1, ctx.dictionary.make_nop())
    }

    #[cmd(name = "'nop")]
    fn interpret_tick_nop(ctx: &mut Context) -> Result<()> {
        ctx.stack.push(ctx.dictionary.make_nop())
    }

    // === Dictionary manipulation ===

    #[cmd(name = "find")]
    fn interpret_find(ctx: &mut Context) -> Result<()> {
        let mut word = ctx.stack.pop_string()?;
        let entry = match ctx.dictionary.lookup(&word) {
            Some(entry) => Some(entry),
            None => {
                word.push(' ');
                ctx.dictionary.lookup(&word)
            }
        };
        match entry {
            Some(entry) => {
                ctx.stack.push(entry.definition.clone())?;
                ctx.stack.push_bool(true)
            }
            None => ctx.stack.push_bool(false),
        }
    }

    #[cmd(name = "create")]
    fn interpret_create(ctx: &mut Context) -> Result<()> {
        // NOTE: same as `:`, but not active
        let cont = ctx.stack.pop_cont()?;
        let name = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;

        define_word(
            &mut ctx.dictionary,
            name.data.to_owned(),
            *cont,
            DefMode {
                active: false,
                prefix: false,
            },
        )
    }

    #[cmd(name = "(create)", args(mode = None))]
    fn interpret_create_aux(ctx: &mut Context, mode: Option<DefMode>) -> Result<()> {
        let mode = match mode {
            Some(mode) => mode,
            None => {
                let flags = ctx.stack.pop_smallint_range(0, 3)?;
                DefMode {
                    active: flags & 0b01 != 0,
                    prefix: flags & 0b10 != 0,
                }
            }
        };
        let word = ctx.stack.pop_string()?;
        let cont = ctx.stack.pop_cont()?;
        define_word(&mut ctx.dictionary, *word, *cont, mode)
    }

    #[cmd(name = ":", active, args(active = false, prefix = false))]
    #[cmd(name = "::", active, args(active = true, prefix = false))]
    #[cmd(name = ":_", active, args(active = false, prefix = true))]
    #[cmd(name = "::_", active, args(active = true, prefix = true))]
    fn interpret_colon(ctx: &mut Context, active: bool, prefix: bool) -> Result<()> {
        let cont = ctx.stack.pop_cont()?;
        let name = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;

        define_word(
            &mut ctx.dictionary,
            name.data.to_owned(),
            *cont,
            DefMode { active, prefix },
        )?;

        ctx.stack.push_argcount(0, ctx.dictionary.make_nop())
    }

    #[cmd(name = "forget", args(word_from_stack = false))]
    #[cmd(name = "(forget)", args(word_from_stack = true))]
    fn interpret_forget(ctx: &mut Context, word_from_stack: bool) -> Result<()> {
        let mut word = if word_from_stack {
            *ctx.stack.pop_string()?
        } else {
            let word = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;
            word.data.to_owned()
        };

        if ctx.dictionary.lookup(&word).is_none() {
            word.push(' ');
            if ctx.dictionary.lookup(&word).is_none() {
                anyhow::bail!("Undefined word `{}`", word.trim());
            }
        }

        ctx.dictionary.undefine_word(&word);
        Ok(())
    }

    // === Input parse ===

    #[cmd(name = "word")]
    fn interpret_word(ctx: &mut Context) -> Result<()> {
        let delim = ctx.stack.pop_smallint_char()?;
        let token = if delim.is_whitespace() {
            ctx.input.scan_until_space_or_eof()
        } else {
            ctx.input.scan_until_delimiter(delim)
        }?;
        ctx.stack.push(token.data.to_owned())
    }

    #[cmd(name = "skipspc")]
    fn interpret_skipspc(ctx: &mut Context) -> Result<()> {
        ctx.input.scan_skip_whitespace()
    }

    #[cmd(name = "include", tail)]
    fn interpret_include(ctx: &mut Context) -> Result<Option<Cont>> {
        let name = ctx.stack.pop_string()?;
        let source_block = ctx.env.include(&name)?;
        ctx.input.push_source_block(source_block);
        ctx.next = cont::SeqCont::make(Some(Rc::new(ExitSourceBlockCont)), ctx.next.take());
        Ok(Some(Rc::new(cont::InterpreterCont)))
    }

    #[cmd(name = "skip-to-eof", tail)]
    fn interpret_skip_source(ctx: &mut Context) -> Result<Option<Cont>> {
        let cont = ctx.exit_interpret.fetch();
        ctx.next = None;
        Ok(if !cont.is_null() {
            Some(*cont.into_cont()?)
        } else {
            None
        })
    }

    #[cmd(name = "abort")]
    fn interpret_abort(ctx: &mut Context) -> Result<()> {
        ctx.stdout.flush()?;
        let reason = *ctx.stack.pop_string()?;
        Err(ExecutionAborted { reason }.into())
    }

    #[cmd(name = "quit")]
    fn interpret_quit(ctx: &mut Context) -> Result<()> {
        ctx.exit_code = 0;
        ctx.next = None;
        Ok(())
    }

    #[cmd(name = "bye")]
    fn interpret_bye(ctx: &mut Context) -> Result<()> {
        ctx.exit_code = u8::MAX;
        ctx.next = None;
        Ok(())
    }

    #[cmd(name = "halt")]
    fn interpret_halt(ctx: &mut Context) -> Result<()> {
        ctx.exit_code = ctx.stack.pop_smallint_range(0, 255)? as u8;
        ctx.next = None;
        Ok(())
    }
}

fn define_word(d: &mut Dictionary, mut word: String, cont: Cont, mode: DefMode) -> Result<()> {
    anyhow::ensure!(!word.is_empty(), "Word definition is empty");
    if !mode.prefix {
        word.push(' ');
    }
    d.define_word(
        word,
        DictionaryEntry {
            definition: cont,
            active: mode.active,
        },
        true,
    )
}

#[derive(Default)]
struct DefMode {
    active: bool,
    prefix: bool,
}

struct ExitInterpretCont;

impl cont::ContImpl for ExitInterpretCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        ctx.stack.push(ctx.exit_interpret.clone())?;
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("'exit-interpret")
    }
}

struct ExitSourceBlockCont;

impl cont::ContImpl for ExitSourceBlockCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        ctx.input.pop_source_block();
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<exit source block>")
    }
}
