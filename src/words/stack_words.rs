use num_bigint::BigInt;
use num_traits::Zero;

use crate::dictionary::*;
use crate::error::*;
use crate::stack::*;

pub fn init(d: &mut Dictionary) -> FiftResult<()> {
    words!(d, {
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
    });
    Ok(())
}

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
