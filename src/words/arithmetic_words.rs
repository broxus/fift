use std::rc::Rc;

use crate::continuation::*;
use crate::dictionary::*;
use crate::error::*;
use crate::stack::*;

pub fn init(d: &mut Dictionary) -> FiftResult<()> {
    words!(d, {
        @stk "+" => interpret_plus,
        @stk "-" => interpret_minus,
        @stk "negate" => interpret_negate,
        @stk "1+" => |s| interpret_plus_imm(s, 1),
        @stk "1-" => |s| interpret_plus_imm(s, -1),
        @stk "2+" => |s| interpret_plus_imm(s, 2),
        @stk "2-" => |s| interpret_plus_imm(s, -1),
        @stk "*" => interpret_times,

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
    });
    Ok(())
}

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
