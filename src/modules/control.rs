use std::rc::Rc;

use crate::core::*;
use crate::error::*;

pub struct Control;

#[fift_module]
impl Control {
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

        // TODO
        Ok(())
    }

    #[cmd(name = "}", active)]
    fn interpret_wordlist_end(ctx: &mut Context) -> Result<()> {
        // TODO
        ctx.state.end_compile()
    }

    #[cmd(name = "(compile)")]
    fn interpret_compile_internal(ctx: &mut Context) -> Result<()> {
        ctx.stack.pop_compile()
    }

    #[cmd(name = "(execute)", tail)]
    fn interpret_execute_internal(ctx: &mut Context) -> Result<Option<Cont>> {
        let cont = ctx.stack.pop_argcount()?;
        Ok(Some(cont))
    }

    #[cmd(name = "'", active)]
    fn interpret_tick(ctx: &mut Context) -> Result<()> {
        let word = ctx.input.scan_word()?.ok_or(Error::UnexpectedEof)?;
        let entry = match ctx.dictionary.lookup(word.data) {
            Some(entry) => entry,
            None => {
                let word = format!("{} ", word.data);
                ctx.dictionary.lookup(&word).ok_or(Error::UndefinedWord)?
            }
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

    // === Input parse ===

    #[cmd(name = "abort")]
    fn interpret_abort(ctx: &mut Context) -> Result<()> {
        let _string = ctx.stack.pop_string()?;
        Err(Error::ExecutionAborted)
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
