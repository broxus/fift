use std::rc::Rc;

use anyhow::{Context as _, Result};

use crate::core::*;
use crate::error::{ExecutionAborted, UnexpectedEof};

pub struct Control;

#[fift_module]
impl Control {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        d.define_word("'exit-interpret ", Rc::new(ExitInterpretCont))?;

        d.define_word(
            "Fift-wordlist ",
            Rc::new(cont::LitCont(d.get_words_box().clone())),
        )?;
        d.define_word(
            "Fift ",
            Rc::new(ResetContextCont(d.get_words_box().clone())),
        )?;

        Ok(())
    }

    // === Execution control ===

    #[cmd(name = "execute", tail)]
    fn interpret_execute(ctx: &mut Context) -> Result<Option<Cont>> {
        let cont = ctx.stack.pop_cont()?;
        Ok(Some(cont.as_ref().clone()))
    }

    #[cmd(name = "call/cc", tail)]
    fn interpret_call_cc(ctx: &mut Context) -> Result<Option<Cont>> {
        let next = ctx.stack.pop_cont()?;
        if let Some(next) = ctx.next.take() {
            ctx.stack.push(next)?;
        } else {
            ctx.stack.push_null()?;
        }
        Ok(Some(next.as_ref().clone()))
    }

    #[cmd(name = "times", tail)]
    fn interpret_execute_times(ctx: &mut Context) -> Result<Option<Cont>> {
        let count = ctx.stack.pop_smallint_range(0, 1000000000)? as usize;
        let body = ctx.stack.pop_cont_owned()?;
        Ok(match count {
            0 => None,
            1 => Some(body),
            _ => {
                ctx.next = Some(Rc::new(cont::TimesCont {
                    body: Some(body.clone()),
                    after: ctx.next.take(),
                    count: count - 1,
                }));
                Some(body)
            }
        })
    }

    #[cmd(name = "if", tail)]
    fn interpret_if(ctx: &mut Context) -> Result<Option<Cont>> {
        let true_ref = ctx.stack.pop_cont()?;
        Ok(if ctx.stack.pop_bool()? {
            Some(true_ref.as_ref().clone())
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
            Some(false_ref.as_ref().clone())
        })
    }

    #[cmd(name = "cond", tail)]
    fn interpret_cond(ctx: &mut Context) -> Result<Option<Cont>> {
        let false_ref = ctx.stack.pop_cont()?;
        let true_ref = ctx.stack.pop_cont()?;
        Ok(Some(if ctx.stack.pop_bool()? {
            true_ref.as_ref().clone()
        } else {
            false_ref.as_ref().clone()
        }))
    }

    #[cmd(name = "while", tail)]
    fn interpret_while(ctx: &mut Context) -> Result<Option<Cont>> {
        let body = ctx.stack.pop_cont_owned()?;
        let cond = ctx.stack.pop_cont_owned()?;
        ctx.next = Some(Rc::new(cont::WhileCont {
            condition: Some(cond.clone()),
            body: Some(body),
            after: ctx.next.take(),
            running_body: true,
        }));
        Ok(Some(cond))
    }

    #[cmd(name = "until", tail)]
    fn interpret_until(ctx: &mut Context) -> Result<Option<Cont>> {
        let body = ctx.stack.pop_cont_owned()?;
        ctx.next = Some(Rc::new(cont::UntilCont {
            body: Some(body.clone()),
            after: ctx.next.take(),
        }));
        Ok(Some(body))
    }

    // === Compiler control ===

    #[cmd(name = "[", active)]
    fn interpret_internal_interpret_begin(ctx: &mut Context) -> Result<()> {
        ctx.state.begin_interpret_internal()?;
        ctx.stack.push_argcount(0)
    }

    #[cmd(name = "]", active)]
    fn interpret_internal_interpret_end(ctx: &mut Context) -> Result<()> {
        ctx.state.end_interpret_internal()?;
        ctx.stack.push_raw(cont::NopCont::value_instance())
    }

    #[cmd(name = "{", active)]
    fn interpret_wordlist_begin(ctx: &mut Context) -> Result<()> {
        ctx.state.begin_compile()?;
        interpret_wordlist_begin_aux(&mut ctx.stack)?;
        ctx.stack.push_argcount(0)
    }

    #[cmd(name = "}", active)]
    fn interpret_wordlist_end(ctx: &mut Context) -> Result<()> {
        ctx.state.end_compile()?;
        interpret_wordlist_end_aux(ctx)?;
        ctx.stack.push_argcount(1)
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
        let word = ctx.input.scan_word()?.ok_or(UnexpectedEof)?.to_owned();
        let entry = ctx
            .dicts
            .lookup(&word, true)?
            .with_context(|| format!("Undefined word `{word}`"))?;
        ctx.stack.push(entry.definition.clone())?;
        ctx.stack.push_argcount(1)
    }

    #[cmd(name = "'nop")]
    fn interpret_tick_nop(ctx: &mut Context) -> Result<()> {
        ctx.stack.push_raw(cont::NopCont::value_instance())
    }

    // === Dictionary manipulation ===

    #[cmd(name = "find")]
    fn interpret_find(ctx: &mut Context) -> Result<()> {
        let word = ctx.stack.pop_string()?;
        match ctx.dicts.lookup(&word, true)? {
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
        let word = ctx.input.scan_word()?.ok_or(UnexpectedEof)?.to_owned();

        define_word(
            &mut ctx.dicts.current,
            word,
            cont.as_ref().clone(),
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
        let word = ctx.stack.pop_string_owned()?;
        let cont = ctx.stack.pop_cont_owned()?;
        define_word(&mut ctx.dicts.current, word, cont, mode)
    }

    #[cmd(name = ":", active, args(active = false, prefix = false))]
    #[cmd(name = "::", active, args(active = true, prefix = false))]
    #[cmd(name = ":_", active, args(active = false, prefix = true))]
    #[cmd(name = "::_", active, args(active = true, prefix = true))]
    fn interpret_colon(ctx: &mut Context, active: bool, prefix: bool) -> Result<()> {
        thread_local! {
            static CREATE_AUX: Cont = Rc::new((|ctx| interpret_create_aux(ctx, None)) as cont::ContextWordFunc);
        };

        let name = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;
        let mode = (active as u8) | (prefix as u8) << 1;

        let cont = CREATE_AUX.with(|cont| cont.clone());

        ctx.stack.push(name.to_owned())?;
        ctx.stack.push_int(mode)?;
        ctx.stack.push_int(2)?;
        ctx.stack.push(cont)
    }

    #[cmd(name = "forget", args(word_from_stack = false))]
    #[cmd(name = "(forget)", args(word_from_stack = true))]
    fn interpret_forget(ctx: &mut Context, word_from_stack: bool) -> Result<()> {
        let mut word = if word_from_stack {
            ctx.stack.pop_string_owned()?
        } else {
            ctx.input.scan_word()?.ok_or(UnexpectedEof)?.to_owned()
        };

        if ctx.dicts.current.lookup(&word)?.is_none() {
            word.push(' ');
            if ctx.dicts.current.lookup(&word)?.is_none() {
                anyhow::bail!("Undefined word `{}`", word.trim());
            }
        }

        ctx.dicts.current.undefine_word(&word)?;
        Ok(())
    }

    #[cmd(name = "current@")]
    fn interpret_get_current(ctx: &mut Context) -> Result<()> {
        let words = ctx.dicts.current.get_words_box().clone();
        ctx.stack.push_raw(words)
    }

    #[cmd(name = "current!")]
    fn interpret_set_current(ctx: &mut Context) -> Result<()> {
        let words = ctx.stack.pop_shared_box()?;
        ctx.dicts.current.set_words_box(words);
        Ok(())
    }

    #[cmd(name = "context@")]
    fn interpret_get_context(ctx: &mut Context) -> Result<()> {
        let words = ctx.dicts.context.get_words_box().clone();
        ctx.stack.push_raw(words)
    }

    #[cmd(name = "context!")]
    fn interpret_set_context(ctx: &mut Context) -> Result<()> {
        let words = ctx.stack.pop_shared_box()?;
        ctx.dicts.context.set_words_box(words);
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
        ctx.stack.push(token.to_owned())
    }

    #[cmd(name = "(word)")]
    fn interpret_word_ext(ctx: &mut Context) -> Result<()> {
        const MODE_SKIP_SPACE_EOL: u8 = 0b100;
        const MODE_SKIP_SPACE: u8 = 0b1000;

        let mode = ctx.stack.pop_smallint_range(0, 11)? as u8;
        let delims = ctx.stack.pop_string()?;

        // TODO: these flags might be ignored?
        if mode & MODE_SKIP_SPACE != 0 {
            if mode & MODE_SKIP_SPACE_EOL != 0 {
                ctx.input.scan_skip_whitespace()?;
            } else {
                ctx.input.skip_line_whitespace();
            }
        }

        let word = ctx.input.scan_classify(&delims, mode & 0b11)?;
        ctx.stack.push(word.to_owned())
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
            Some(cont.into_cont()?.as_ref().clone())
        } else {
            None
        })
    }

    #[cmd(name = "abort")]
    fn interpret_abort(ctx: &mut Context) -> Result<()> {
        ctx.stdout.flush()?;
        let reason = ctx.stack.pop_string()?.as_ref().clone();
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
    )
}

#[derive(Default)]
struct DefMode {
    active: bool,
    prefix: bool,
}

struct ResetContextCont(Rc<SharedBox>);

impl cont::ContImpl for ResetContextCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        ctx.dicts.context.set_words_box(self.0.clone());
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Fift")
    }
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
