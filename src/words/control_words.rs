use std::rc::Rc;

use crate::context::*;
use crate::continuation::*;
use crate::dictionary::*;
use crate::error::*;

pub fn init(d: &mut Dictionary) -> FiftResult<()> {
    words!(d, {
        // Execution control
        @ctl "execute" => interpret_execute,
        @ctl "times" => interpret_execute_times,
        @ctl "if" => interpret_if,
        @ctl "ifnot" => interpret_ifnot,
        @ctl "cond" => interpret_cond,
        @ctl "while" => interpret_while,
        @ctl "until" => interpret_until,

        // Compiler control
        @act "[" => interpret_internal_interpret_begin,
        @act "]" => interpret_internal_interpret_end,
        @ctx "(compile)" => interpret_compile_internal,
        @ctl "(execute)" => interpret_execute_internal,
        @act "'" => interpret_tick,
        @ctx "'nop" => interpret_tick_nop,

        // Dictionary manipulation
        @ctx "find" => interpret_find,

        // TODO

        // Input parse
        @ctx "abort" => interpret_abort,
        @ctx "quit" => interpret_quit,
        @ctx "bye" => interpret_bye,
        @ctx "halt" => interpret_halt,
    });
    Ok(())
}

// Execution control

fn interpret_execute(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let cont = ctx.stack.pop()?.into_cont()?;
    Ok(Some(*cont))
}

fn interpret_execute_times(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let count = ctx.stack.pop_smallint_range(0, 1000000000)? as usize;
    let body = ctx.stack.pop()?.into_cont()?;
    Ok(match count {
        0 => None,
        1 => Some(*body),
        _ => {
            ctx.next = Some(Rc::new(TimesCont {
                body: Some(Rc::clone(&body)),
                after: ctx.next.take(),
                count: count - 1,
            }));
            Some(*body)
        }
    })
}

fn interpret_if(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let true_ref = ctx.stack.pop()?.into_cont()?;
    Ok(if ctx.stack.pop_bool()? {
        Some(*true_ref)
    } else {
        None
    })
}

fn interpret_ifnot(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let false_ref = ctx.stack.pop()?.into_cont()?;
    Ok(if ctx.stack.pop_bool()? {
        None
    } else {
        Some(*false_ref)
    })
}

fn interpret_cond(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let false_ref = ctx.stack.pop()?.into_cont()?;
    let true_ref = ctx.stack.pop()?.into_cont()?;
    Ok(Some(if ctx.stack.pop_bool()? {
        *true_ref
    } else {
        *false_ref
    }))
}

fn interpret_while(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let body = ctx.stack.pop()?.into_cont()?;
    let cond = ctx.stack.pop()?.into_cont()?;
    ctx.next = Some(Rc::new(WhileCont {
        condition: Some(Rc::clone(&cond)),
        body: Some(*body),
        after: ctx.next.take(),
        running_body: true,
    }));
    Ok(Some(*cond))
}

fn interpret_until(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let body = ctx.stack.pop()?.into_cont()?;
    ctx.next = Some(Rc::new(UntilCont {
        body: Some(Rc::clone(&body)),
        after: ctx.next.take(),
    }));
    Ok(Some(*body))
}

// Compiler control

fn interpret_internal_interpret_begin(ctx: &mut Context) -> FiftResult<()> {
    ctx.state.begin_interpret_internal()?;
    ctx.stack.push_argcount(0, ctx.dictionary.make_nop())
}

fn interpret_internal_interpret_end(ctx: &mut Context) -> FiftResult<()> {
    ctx.state.end_interpret_internal()?;
    ctx.stack.push(Box::new(ctx.dictionary.make_nop()))
}

fn interpret_compile_internal(ctx: &mut Context) -> FiftResult<()> {
    ctx.stack.pop_compile()
}

fn interpret_execute_internal(ctx: &mut Context) -> FiftResult<Option<Continuation>> {
    let cont = ctx.stack.pop_argcount()?;
    Ok(Some(cont))
}

fn interpret_tick(ctx: &mut Context) -> FiftResult<()> {
    let word = ctx.input.scan_word()?.ok_or(FiftError::UnexpectedEof)?;
    let entry = match ctx.dictionary.lookup(&word.data) {
        Some(entry) => entry,
        None => {
            let word = format!("{} ", word.data);
            ctx.dictionary
                .lookup(&word)
                .ok_or(FiftError::UndefinedWord)?
        }
    };
    ctx.stack.push(Box::new(entry.definition.clone()))?;
    ctx.stack.push_argcount(1, ctx.dictionary.make_nop())
}

fn interpret_tick_nop(ctx: &mut Context) -> FiftResult<()> {
    ctx.stack.push(Box::new(ctx.dictionary.make_nop()))
}

fn interpret_find(ctx: &mut Context) -> FiftResult<()> {
    let mut word = ctx.stack.pop()?.into_string()?;
    let entry = match ctx.dictionary.lookup(&word) {
        Some(entry) => Some(entry),
        None => {
            word.push(' ');
            ctx.dictionary.lookup(&word)
        }
    };
    match entry {
        Some(entry) => {
            ctx.stack.push(Box::new(entry.definition.clone()))?;
            ctx.stack.push_bool(true)
        }
        None => ctx.stack.push_bool(false),
    }
}

//

fn interpret_abort(ctx: &mut Context) -> FiftResult<()> {
    let _string = ctx.stack.pop()?.into_string()?;
    Err(FiftError::ExecutionAborted)
}

fn interpret_quit(ctx: &mut Context) -> FiftResult<()> {
    ctx.exit_code = 0;
    ctx.next = None;
    Ok(())
}

fn interpret_bye(ctx: &mut Context) -> FiftResult<()> {
    ctx.exit_code = u8::MAX;
    ctx.next = None;
    Ok(())
}

fn interpret_halt(ctx: &mut Context) -> FiftResult<()> {
    ctx.exit_code = ctx.stack.pop_smallint_range(0, 255)? as u8;
    ctx.next = None;
    Ok(())
}
