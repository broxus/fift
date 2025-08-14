use std::rc::Rc;

use anyhow::{Context as _, Result};
use tycho_vm::SafeRc;

use crate::core::*;
use crate::error::{ExecutionAborted, UnexpectedEof};
use crate::util::ImmediateInt;

pub struct Control;

#[fift_module]
impl Control {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        d.define_word("'exit-interpret ", SafeRc::new(ExitInterpretCont))?;

        d.define_word(
            "Fift-wordlist ",
            SafeRc::new(cont::LitCont(
                d.get_words_box().clone().into_dyn_fift_value(),
            )),
        )?;
        d.define_word(
            "Fift ",
            SafeRc::new(ResetContextCont(d.get_words_box().clone())),
        )?;

        Ok(())
    }

    // === Execution control ===

    #[cmd(name = "execute", tail)]
    fn interpret_execute(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let cont = ctx.stack.pop_cont()?;
        Ok(Some(cont))
    }

    #[cmd(name = "call/cc", tail)]
    fn interpret_call_cc(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let next = ctx.stack.pop_cont()?;
        if let Some(next) = ctx.next.take() {
            ctx.stack.push_raw(next.into_dyn_fift_value())?;
        } else {
            ctx.stack.push_null()?;
        }
        Ok(Some(next))
    }

    #[cmd(name = "times", tail)]
    fn interpret_execute_times(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let count = ctx.stack.pop_smallint_range(0, 1000000000)? as usize;
        let body = ctx.stack.pop_cont()?;
        Ok(match count {
            0 => None,
            1 => Some(body),
            _ => {
                ctx.next = Some(SafeRc::new_dyn_fift_cont(cont::TimesCont {
                    body: Some(body.clone()),
                    after: ctx.next.take(),
                    count: count - 1,
                }));
                Some(body)
            }
        })
    }

    #[cmd(name = "if", tail)]
    fn interpret_if(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let true_ref = ctx.stack.pop_cont()?;
        Ok(if ctx.stack.pop_bool()? {
            Some(true_ref)
        } else {
            None
        })
    }

    #[cmd(name = "ifnot", tail)]
    fn interpret_ifnot(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let false_ref = ctx.stack.pop_cont()?;
        Ok(if ctx.stack.pop_bool()? {
            None
        } else {
            Some(false_ref)
        })
    }

    #[cmd(name = "cond", tail)]
    fn interpret_cond(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let false_ref = ctx.stack.pop_cont()?;
        let true_ref = ctx.stack.pop_cont()?;
        Ok(Some(if ctx.stack.pop_bool()? {
            true_ref
        } else {
            false_ref
        }))
    }

    #[cmd(name = "while", tail)]
    fn interpret_while(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let body = ctx.stack.pop_cont()?;
        let cond = ctx.stack.pop_cont()?;
        ctx.next = Some(RcFiftCont::new_dyn_fift_cont(cont::WhileCont {
            condition: Some(cond.clone()),
            body: Some(body),
            after: ctx.next.take(),
            running_body: true,
        }));
        Ok(Some(cond))
    }

    #[cmd(name = "until", tail)]
    fn interpret_until(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let body = ctx.stack.pop_cont()?;
        ctx.next = Some(SafeRc::new_dyn_fift_cont(cont::UntilCont {
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
        ctx.stack
            .push_raw(SafeRc::into_inner(word_list).finish().into_dyn_fift_value())
    }

    #[cmd(name = "(compile)")]
    fn interpret_compile_internal(ctx: &mut Context) -> Result<()> {
        ctx.compile_stack_top()
    }

    #[cmd(name = "(execute)", tail)]
    fn interpret_execute_internal(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let cont = ctx.execute_stack_top()?;
        Ok(Some(cont))
    }

    #[cmd(name = "(interpret-prepare)", tail)]
    fn interpret_prepare(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let found = ctx.stack.pop_smallint_signed_range(-1, 1)?;
        Ok(if found == 0 {
            // Interpret number
            let string = ctx.stack.pop_string()?;
            let int = ImmediateInt::try_from_str(&string)?
                .with_context(|| format!("Failed to parse `{string}` as number"))?;

            let mut res = 1;
            ctx.stack.push(int.num)?;
            if let Some(denom) = int.denom {
                res += 1;
                ctx.stack.push(denom)?;
            }
            ctx.stack.push_argcount(res)?;
            None
        } else if found == -1 {
            // Interpret ordinary word
            ctx.stack.push_int(0)?;
            ctx.stack.swap(0, 1)?;
            None
        } else {
            // Interpret active word
            Some(ctx.stack.pop_cont()?)
        })
    }

    #[cmd(name = "'", active)]
    fn interpret_tick(ctx: &mut Context) -> Result<()> {
        let word = ctx.input.scan_word()?.ok_or(UnexpectedEof)?.to_owned();
        let entry = ctx
            .dicts
            .lookup(&word, true)?
            .with_context(|| format!("Undefined word `{word}`"))?;
        ctx.stack
            .push_raw(entry.definition.clone().into_dyn_fift_value())?;
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
                ctx.stack
                    .push_raw(entry.definition.clone().into_dyn_fift_value())?;
                ctx.stack.push_bool(true)
            }
            None => ctx.stack.push_bool(false),
        }
    }

    #[cmd(name = "(word-prefix-find)")]
    fn interpret_word_prefix_find(ctx: &mut Context) -> Result<()> {
        let mut rewind = None;
        let (word, entry) = 'entry: {
            let Some(token) = ctx.input.scan_word()? else {
                ctx.stack.push(String::new())?;
                ctx.stack.push_int(0)?;
                return Ok(());
            };

            let mut word = token.to_owned();
            word.push(' ');

            // Search parsed token as a separate word first
            if let Some(entry) = ctx.dicts.lookup(&word, false)? {
                break 'entry (word, Some(entry));
            }

            // Then find the largest possible prefix
            while !word.is_empty() {
                word.pop();
                if let Some(entry) = ctx.dicts.lookup(&word, false)? {
                    rewind = Some(word.len());
                    break 'entry (word, Some(entry));
                }
            }

            // Just push token otherwise
            word.clear();
            word.push_str(token);
            // ctx.input.scan_skip_whitespace()?;
            (word, None)
        };

        if let Some(rewind) = rewind {
            ctx.input.rewind(rewind);
        } else {
            ctx.input.skip_line_whitespace();
        }

        match entry {
            None => {
                ctx.stack.push(word)?;
                ctx.stack.push_int(0)
            }
            Some(entry) => {
                ctx.stack
                    .push_raw(entry.definition.clone().into_dyn_fift_value())?;
                ctx.stack.push_int(if entry.active { 1 } else { -1 })
            }
        }
    }

    #[cmd(name = "create")]
    fn interpret_create(ctx: &mut Context) -> Result<()> {
        // NOTE: same as `:`, but not active
        let cont = ctx.stack.pop_cont()?;
        let word = ctx.input.scan_word()?.ok_or(UnexpectedEof)?.to_owned();

        define_word(&mut ctx.dicts.current, word, cont, DefMode {
            active: false,
            prefix: false,
        })
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
        let cont = ctx.stack.pop_cont()?;
        define_word(&mut ctx.dicts.current, word, cont, mode)
    }

    #[cmd(name = ":", active, args(active = false, prefix = false))]
    #[cmd(name = "::", active, args(active = true, prefix = false))]
    #[cmd(name = ":_", active, args(active = false, prefix = true))]
    #[cmd(name = "::_", active, args(active = true, prefix = true))]
    fn interpret_colon(ctx: &mut Context, active: bool, prefix: bool) -> Result<()> {
        thread_local! {
            static CREATE_AUX: RcFiftCont = RcFiftCont::new_dyn_fift_cont(
                (|ctx| interpret_create_aux(ctx, None)) as cont::ContextWordFunc
            );
        };

        let name = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;
        let mode = (active as u8) | (prefix as u8) << 1;

        let cont = CREATE_AUX.with(|cont| cont.clone());

        ctx.stack.push(name.to_owned())?;
        ctx.stack.push_int(mode)?;
        ctx.stack.push_int(2)?;
        ctx.stack.push_raw(cont.into_dyn_fift_value())
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
        ctx.stack.push_raw(words.into_dyn_fift_value())
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
        ctx.stack.push_raw(words.into_dyn_fift_value())
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
        ctx.input.scan_skip_whitespace()?;
        Ok(())
    }

    #[cmd(name = "seekeof?", args(mode = 1))]
    #[cmd(name = "(seekeof?)", args(mode = -1))]
    fn interpret_seekeof(ctx: &mut Context, mut mode: i32) -> Result<()> {
        if mode == -1 {
            mode = ctx.stack.pop_smallint_signed_range(-1, 3)?;
        }
        _ = mode; // NOTE: unused
        let eof = !ctx.input.scan_skip_whitespace()?;
        ctx.stack.push_bool(eof)
    }

    #[cmd(name = "include-depth")]
    fn interpret_include_depth(ctx: &mut Context) -> Result<()> {
        ctx.stack.push_int(ctx.input.depth())
    }

    #[cmd(name = "include", tail)]
    fn interpret_include(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let name = ctx.stack.pop_string()?;
        let source_block = ctx.env.include(&name)?;
        ctx.input.push_source_block(source_block);

        if let Some(max_include_depth) = ctx.limits.max_include_depth {
            anyhow::ensure!(
                ctx.input.depth() <= max_include_depth as i32,
                "Max include depth exceeded: {max_include_depth}/{max_include_depth}"
            );
        }

        ctx.next = cont::SeqCont::make(
            Some(RcFiftCont::new_dyn_fift_cont(ExitSourceBlockCont)),
            ctx.next.take(),
        );
        Ok(Some(RcFiftCont::new_dyn_fift_cont(cont::InterpreterCont)))
    }

    #[cmd(name = "skip-to-eof", tail)]
    fn interpret_skip_source(ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let cont = ctx.exit_interpret.fetch();
        ctx.next = None;
        Ok(if !cont.is_null() {
            Some(cont.into_cont()?)
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

fn define_word(
    d: &mut Dictionary,
    mut word: String,
    cont: RcFiftCont,
    mode: DefMode,
) -> Result<()> {
    anyhow::ensure!(!word.is_empty(), "Word definition is empty");
    if !mode.prefix {
        word.push(' ');
    }
    d.define_word(word, DictionaryEntry {
        definition: cont,
        active: mode.active,
    })
}

#[derive(Default)]
struct DefMode {
    active: bool,
    prefix: bool,
}

#[derive(Clone)]
struct ResetContextCont(SafeRc<SharedBox>);

impl cont::FiftCont for ResetContextCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        ctx.dicts.context.set_words_box(self.0.clone());
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Fift")
    }
}

#[derive(Clone, Copy)]
struct ExitInterpretCont;

impl cont::FiftCont for ExitInterpretCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        ctx.stack.push(ctx.exit_interpret.clone())?;
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("'exit-interpret")
    }
}

#[derive(Clone, Copy)]
struct ExitSourceBlockCont;

impl cont::FiftCont for ExitSourceBlockCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        ctx.input.pop_source_block();
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<exit source block>")
    }
}
