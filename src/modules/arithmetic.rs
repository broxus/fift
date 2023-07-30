use std::rc::Rc;

use crate::core::*;
use crate::error::*;

pub struct Arithmetic;

#[fift_module]
impl Arithmetic {
    #[init]
    fn init(d: &mut Dictionary) -> Result<()> {
        let mut make_int_lit = |name: &str, value: i32| {
            d.define_word(
                name,
                DictionaryEntry::new_ordinary(Rc::new(cont::IntLitCont::from(value))),
                false,
            )
        };

        make_int_lit("false ", 0)?;
        make_int_lit("true ", -1)?;
        make_int_lit("0 ", 0)?;
        make_int_lit("1 ", 1)?;
        make_int_lit("2 ", 2)?;
        make_int_lit("-1 ", -1)?;
        make_int_lit("bl ", 32)
    }

    #[cmd(name = "+", stack)]
    fn interpret_plus(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        let rhs = stack.pop_int()?;
        *lhs += *rhs;
        stack.push_raw(lhs)
    }

    #[cmd(name = "-", stack)]
    fn interpret_minus(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        let rhs = stack.pop_int()?;
        *lhs -= *rhs;
        stack.push_raw(lhs)
    }

    #[cmd(name = "1+", stack, args(rhs = 1))]
    #[cmd(name = "1-", stack, args(rhs = -1))]
    #[cmd(name = "2+", stack, args(rhs = 2))]
    #[cmd(name = "2-", stack, args(rhs = -2))]
    fn interpret_plus_imm(stack: &mut Stack, rhs: i32) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        *lhs += rhs;
        stack.push_raw(lhs)
    }

    #[cmd(name = "negate", stack)]
    fn interpret_negate(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        *lhs = -std::mem::take(&mut lhs);
        stack.push_raw(lhs)
    }

    #[cmd(name = "*", stack)]
    fn interpret_mul(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        let rhs = stack.pop_int()?;
        *lhs *= *rhs;
        stack.push_raw(lhs)
    }

    // TODO: other

    #[cmd(name = "not", stack)]
    fn interpret_not(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        *lhs = !std::mem::take(&mut lhs);
        stack.push_raw(lhs)
    }

    #[cmd(name = "and", stack)]
    fn interpret_and(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        let rhs = stack.pop_int()?;
        *lhs &= *rhs;
        stack.push_raw(lhs)
    }

    #[cmd(name = "or", stack)]
    fn interpret_or(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        let rhs = stack.pop_int()?;
        *lhs |= *rhs;
        stack.push_raw(lhs)
    }

    #[cmd(name = "xor", stack)]
    fn interpret_xor(stack: &mut Stack) -> Result<()> {
        let mut lhs = stack.pop_int()?;
        let rhs = stack.pop_int()?;
        *lhs ^= *rhs;
        stack.push_raw(lhs)
    }
}
