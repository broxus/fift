use std::rc::Rc;

use everscale_types::cell::OwnedCellSlice;
use everscale_types::prelude::*;
use num_bigint::BigInt;
use num_traits::Zero;

use crate::context::*;
use crate::continuation::*;
use crate::dictionary::*;
use crate::error::*;
use crate::stack::*;

impl Context<'_> {
    pub fn init_common_words(&mut self) -> FiftResult<()> {
        let d: &mut Dictionary = &mut self.dictionary;

        words!(d, {
            @raw "nop" => DictionaryEntry::new_ordinary(d.make_nop()),
            // Stack print/dump words
            @ctx "." => |c| interpret_dot(c, true),
            @ctx "._" => |c| interpret_dot(c, false),
            @ctx "x." => |c| interpret_dothex(c, false, true),
            @ctx "x._" => |c| interpret_dothex(c, false, false),
            @ctx "X." => |c| interpret_dothex(c, true, true),
            @ctx "X._" => |c| interpret_dothex(c, true, false),
            @ctx "b." => |c| interpret_dotbin(c, true),
            @ctx "b._" => |c| interpret_dotbin(c, false),
            // TODO: csr.
            @ctx ".s" => interpret_dotstack,
            @ctx ".sl" => interpret_dotstack_list,
            @ctx ".dump" => interpret_dump,
            @ctx ".l" => interpret_print_list,
            @stk "(dump)" => interpret_dump_internal,
            @stk "(ldump)" => interpret_list_dump_internal,
            @stk "(.)" => interpret_dot_internal,
            @stk "(x.)" => |s| interpret_dothex_internal(s, false),
            @stk "(X.)" => |s| interpret_dothex_internal(s, true),
            @stk "(b.)" => interpret_dotbin_internal,

            // Stack manipulation
            @stk "drop" => interpret_drop,
            @stk "2drop" => interpret_2drop,
            @stk "dup" => interpret_dup,
            @stk "2dup" => interpret_2dup,
            @stk "over" => interpret_over,
            @stk "2over" => interpret_2over,
            @stk "swap" => interpret_swap,
            @stk "2swap" => interpret_2swap,
            @stk "tuck" => interpret_tuck,
            @stk "nip" => interpret_nip,
            @stk "rot" => interpret_rot,
            @stk "-rot" => interpret_rot_rev,
            @stk "pick" => interpret_pick,
            @stk "roll" => interpret_roll,
            @stk "-roll" => interpret_roll_rev,
            @stk "reverse" => interpret_reverse,
            @stk "exch" => interpret_exch,
            @stk "exch2" => interpret_exch2,
            @stk "depth" => interpret_depth,
            @stk "?dup" => interpret_cond_dup,

            // Arithmetic
            @stk "+" => interpret_plus,
            @stk "-" => interpret_minus,
            @stk "negate" => interpret_negate,
            @stk "1+" => |s| interpret_plus_imm(s, 1),
            @stk "1-" => |s| interpret_plus_imm(s, -1),
            @stk "2+" => |s| interpret_plus_imm(s, 2),
            @stk "2-" => |s| interpret_plus_imm(s, -1),
            @stk "*" => interpret_times,
            // TODO: other

            // Logical
            @stk "not" => interpret_not,
            @stk "and" => interpret_and,
            @stk "or" => interpret_or,
            @stk "xor" => interpret_xor,

            // Integer constants
            @raw "false" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(0))),
            @raw "true" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(-1))),
            @raw "0" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(0))),
            @raw "1" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(1))),
            @raw "2" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(2))),
            @raw "-1" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(-1))),
            @raw "bl" => DictionaryEntry::new_ordinary(Rc::new(IntLitCont::from(32))),

            // Other
            @stk "<b" => interpret_empty,
            @stk "b>" => |s| interpret_store_end(s, false),
            @stk "b>spec" => |s| interpret_store_end(s, true),
            @stk "<s" => interpret_from_cell,
            @stk "s>" => interpret_cell_check_empty,

            // Compiler control
            @act "[" => interpret_internal_interpret_begin,
            @act "]" => interpret_internal_interpret_end,
            @ctx "(compile)" => |c| c.interpret_compile(),
            @ctl "(execute)" => |c| c.interpret_execute(),

            // Input parse
            @ctx "abort" => interpret_abort,
            @ctx "quit" => interpret_quit,
            @ctx "bye" => interpret_bye,
            @ctx "halt" => interpret_halt,
        });
        Ok(())
    }
}

// Stack print/dump words

fn interpret_dot(ctx: &mut Context, space_after: bool) -> FiftResult<()> {
    let item = ctx.stack.pop()?.into_int()?;
    write!(ctx.stdout, "{item}{}", opt_space(space_after))?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dothex(ctx: &mut Context, uppercase: bool, space_after: bool) -> FiftResult<()> {
    let item = ctx.stack.pop()?.into_int()?;
    let space = opt_space(space_after);
    if uppercase {
        write!(ctx.stdout, "{:X}{space}", item.as_ref())
    } else {
        write!(ctx.stdout, "{:x}{space}", item.as_ref())
    }?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dotbin(ctx: &mut Context, space_after: bool) -> FiftResult<()> {
    let item = ctx.stack.pop()?.into_int()?;
    write!(ctx.stdout, "{:b}{}", item.as_ref(), opt_space(space_after))?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dotstack(ctx: &mut Context) -> FiftResult<()> {
    writeln!(ctx.stdout, "{}", ctx.stack.display_dump())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dotstack_list(ctx: &mut Context) -> FiftResult<()> {
    writeln!(ctx.stdout, "{}", ctx.stack.display_list())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dump(ctx: &mut Context) -> FiftResult<()> {
    let item = ctx.stack.pop()?;
    write!(ctx.stdout, "{} ", item.display_dump())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_print_list(ctx: &mut Context) -> FiftResult<()> {
    let item = ctx.stack.pop()?;
    write!(ctx.stdout, "{} ", item.display_list())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dump_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?;
    stack.push(Box::new(item.display_dump().to_string()))
}

fn interpret_list_dump_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?;
    stack.push(Box::new(item.display_list().to_string()))
}

fn interpret_dot_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_int()?;
    stack.push(Box::new(item.to_string()))
}

fn interpret_dothex_internal(stack: &mut Stack, upper: bool) -> FiftResult<()> {
    let item = stack.pop()?.into_int()?;
    let item = if upper {
        format!("{:x}", item.as_ref())
    } else {
        format!("{:X}", item.as_ref())
    };
    stack.push(Box::new(item))
}

fn interpret_dotbin_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_int()?;
    let item = format!("{:b}", item.as_ref());
    stack.push(Box::new(item))
}

// Stack manipulation

fn interpret_drop(stack: &mut Stack) -> FiftResult<()> {
    stack.pop()?;
    Ok(())
}

fn interpret_2drop(stack: &mut Stack) -> FiftResult<()> {
    stack.pop()?;
    stack.pop()?;
    Ok(())
}

fn interpret_dup(stack: &mut Stack) -> FiftResult<()> {
    stack.push(stack.fetch(0)?)
}

fn interpret_2dup(stack: &mut Stack) -> FiftResult<()> {
    stack.push(stack.fetch(0)?)?;
    stack.push(stack.fetch(0)?)
}

fn interpret_over(stack: &mut Stack) -> FiftResult<()> {
    stack.push(stack.fetch(1)?)
}

fn interpret_2over(stack: &mut Stack) -> FiftResult<()> {
    stack.push(stack.fetch(3)?)?;
    stack.push(stack.fetch(3)?)
}

fn interpret_swap(stack: &mut Stack) -> FiftResult<()> {
    stack.swap(0, 1)
}

fn interpret_2swap(stack: &mut Stack) -> FiftResult<()> {
    stack.swap(0, 2)?;
    stack.swap(1, 3)
}

fn interpret_tuck(stack: &mut Stack) -> FiftResult<()> {
    stack.swap(0, 1)?;
    stack.push(stack.fetch(1)?)
}

fn interpret_nip(stack: &mut Stack) -> FiftResult<()> {
    stack.swap(0, 1)?;
    stack.pop()?;
    Ok(())
}

fn interpret_rot(stack: &mut Stack) -> FiftResult<()> {
    stack.swap(1, 2)?;
    stack.swap(0, 1)
}

fn interpret_rot_rev(stack: &mut Stack) -> FiftResult<()> {
    stack.swap(0, 1)?;
    stack.swap(1, 2)
}

fn interpret_pick(stack: &mut Stack) -> FiftResult<()> {
    let n = stack.pop_smallint_range(0, 255)? as usize;
    stack.push(stack.fetch(n)?)
}

fn interpret_roll(stack: &mut Stack) -> FiftResult<()> {
    let n = stack.pop_smallint_range(0, 255)? as usize;
    for i in (1..=n).rev() {
        stack.swap(i, i - 1)?;
    }
    Ok(())
}

fn interpret_roll_rev(stack: &mut Stack) -> FiftResult<()> {
    let n = stack.pop_smallint_range(0, 255)? as usize;
    for i in 0..n {
        stack.swap(i, i + 1)?;
    }
    Ok(())
}

fn interpret_reverse(stack: &mut Stack) -> FiftResult<()> {
    let m = stack.pop_smallint_range(0, 255)? as usize;
    let n = stack.pop_smallint_range(0, 255)? as usize;
    if n == 0 {
        return Ok(());
    }

    stack.check_underflow(n + m)?;
    let s = 2 * m + n - 1;
    for i in (m..=(s - 1) >> 1).rev() {
        stack.swap(i, s - i)?;
    }
    Ok(())
}

fn interpret_exch(stack: &mut Stack) -> FiftResult<()> {
    let n = stack.pop_smallint_range(0, 255)? as usize;
    stack.swap(0, n)
}

fn interpret_exch2(stack: &mut Stack) -> FiftResult<()> {
    let n = stack.pop_smallint_range(0, 255)? as usize;
    let m = stack.pop_smallint_range(0, 255)? as usize;
    stack.swap(n, m)
}

fn interpret_depth(stack: &mut Stack) -> FiftResult<()> {
    stack.push(Box::new(BigInt::from(stack.depth())))
}

fn interpret_cond_dup(stack: &mut Stack) -> FiftResult<()> {
    if !stack.pop()?.as_int()?.is_zero() {
        stack.push(stack.fetch(0)?)?;
    }
    Ok(())
}

// Arithmetic

fn interpret_plus(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    let rhs = stack.pop()?.into_int()?;
    *lhs += *rhs;
    stack.push(lhs)
}

fn interpret_minus(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    let rhs = stack.pop()?.into_int()?;
    *lhs -= *rhs;
    stack.push(lhs)
}

fn interpret_plus_imm(stack: &mut Stack, rhs: i32) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    *lhs += rhs;
    stack.push(lhs)
}

fn interpret_negate(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    *lhs = -std::mem::take(&mut lhs);
    stack.push(lhs)
}

fn interpret_times(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    let rhs = stack.pop()?.into_int()?;
    *lhs *= *rhs;
    stack.push(lhs)
}

// TODO: other

// Logical

fn interpret_not(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    *lhs = !std::mem::take(&mut lhs);
    stack.push(lhs)
}

fn interpret_and(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    let rhs = stack.pop()?.into_int()?;
    *lhs &= *rhs;
    stack.push(lhs)
}

fn interpret_or(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    let rhs = stack.pop()?.into_int()?;
    *lhs |= *rhs;
    stack.push(lhs)
}

fn interpret_xor(stack: &mut Stack) -> FiftResult<()> {
    let mut lhs = stack.pop()?.into_int()?;
    let rhs = stack.pop()?.into_int()?;
    *lhs ^= *rhs;
    stack.push(lhs)
}

// Cells

fn interpret_empty(stack: &mut Stack) -> FiftResult<()> {
    stack.push(Box::new(CellBuilder::new()))
}

fn interpret_store_end(stack: &mut Stack, is_exotic: bool) -> FiftResult<()> {
    let mut item = stack.pop()?.into_builder()?;
    item.set_exotic(is_exotic);
    let cell = item.build()?;
    stack.push(Box::new(cell))
}

fn interpret_from_cell(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_cell()?;
    let slice = OwnedCellSlice::new(*item)?;
    stack.push(Box::new(slice))
}

fn interpret_cell_check_empty(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_slice()?;
    let item = item.as_ref().as_ref();
    if !item.is_data_empty() || !item.is_refs_empty() {
        return Err(FiftError::ExpectedEmptySlice);
    }
    Ok(())
}

// Control

fn interpret_internal_interpret_begin(ctx: &mut Context) -> FiftResult<()> {
    ctx.state.begin_interpret_internal()?;
    ctx.stack.push_argcount(0, ctx.dictionary.make_nop())
}

fn interpret_internal_interpret_end(ctx: &mut Context) -> FiftResult<()> {
    ctx.state.end_interpret_internal()?;
    ctx.stack.push(Box::new(ctx.dictionary.make_nop()))
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

const fn opt_space(space_after: bool) -> &'static str {
    if space_after {
        " "
    } else {
        ""
    }
}
