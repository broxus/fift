use std::rc::Rc;

use anyhow::Result;
use num_traits::Zero;

use crate::core::*;

pub struct StackUtils;

#[fift_module]
impl StackUtils {
    #[cmd(name = "drop", stack)]
    fn interpret_drop(stack: &mut Stack) -> Result<()> {
        stack.pop()?;
        Ok(())
    }

    #[cmd(name = "2drop", stack)]
    fn interpret_2drop(stack: &mut Stack) -> Result<()> {
        stack.pop()?;
        stack.pop()?;
        Ok(())
    }

    #[cmd(name = "dup", stack)]
    fn interpret_dup(stack: &mut Stack) -> Result<()> {
        stack.push_raw(stack.fetch(0)?)
    }

    #[cmd(name = "2dup", stack)]
    fn interpret_2dup(stack: &mut Stack) -> Result<()> {
        stack.push_raw(stack.fetch(1)?)?;
        stack.push_raw(stack.fetch(1)?)
    }

    #[cmd(name = "over", stack)]
    fn interpret_over(stack: &mut Stack) -> Result<()> {
        stack.push_raw(stack.fetch(1)?)
    }

    #[cmd(name = "2over", stack)]
    fn interpret_2over(stack: &mut Stack) -> Result<()> {
        stack.push_raw(stack.fetch(3)?)?;
        stack.push_raw(stack.fetch(3)?)
    }

    #[cmd(name = "swap", stack)]
    fn interpret_swap(stack: &mut Stack) -> Result<()> {
        stack.swap(0, 1)
    }

    #[cmd(name = "2swap", stack)]
    fn interpret_2swap(stack: &mut Stack) -> Result<()> {
        stack.swap(0, 2)?;
        stack.swap(1, 3)
    }

    #[cmd(name = "tuck", stack)]
    fn interpret_tuck(stack: &mut Stack) -> Result<()> {
        stack.swap(0, 1)?;
        stack.push_raw(stack.fetch(1)?)
    }

    #[cmd(name = "nip", stack)]
    fn interpret_nip(stack: &mut Stack) -> Result<()> {
        stack.swap(0, 1)?;
        stack.pop()?;
        Ok(())
    }

    #[cmd(name = "rot", stack)]
    fn interpret_rot(stack: &mut Stack) -> Result<()> {
        stack.swap(1, 2)?;
        stack.swap(0, 1)
    }

    #[cmd(name = "-rot", stack)]
    fn interpret_rot_rev(stack: &mut Stack) -> Result<()> {
        stack.swap(0, 1)?;
        stack.swap(1, 2)
    }

    #[cmd(name = "pick", stack)]
    fn interpret_pick(stack: &mut Stack) -> Result<()> {
        let n = stack.pop_smallint_range(0, 255)? as usize;
        stack.push_raw(stack.fetch(n)?)
    }

    #[cmd(name = "roll", stack)]
    fn interpret_roll(stack: &mut Stack) -> Result<()> {
        let n = stack.pop_smallint_range(0, 255)? as usize;
        for i in (1..=n).rev() {
            stack.swap(i, i - 1)?;
        }
        Ok(())
    }

    #[cmd(name = "-roll", stack)]
    fn interpret_roll_rev(stack: &mut Stack) -> Result<()> {
        let n = stack.pop_smallint_range(0, 255)? as usize;
        for i in 0..n {
            stack.swap(i, i + 1)?;
        }
        Ok(())
    }

    #[cmd(name = "reverse", stack)]
    fn interpret_reverse(stack: &mut Stack) -> Result<()> {
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

    #[cmd(name = "exch", stack)]
    fn interpret_exch(stack: &mut Stack) -> Result<()> {
        let n = stack.pop_smallint_range(0, 255)? as usize;
        stack.swap(0, n)
    }

    #[cmd(name = "exch2", stack)]
    fn interpret_exch2(stack: &mut Stack) -> Result<()> {
        let n = stack.pop_smallint_range(0, 255)? as usize;
        let m = stack.pop_smallint_range(0, 255)? as usize;
        stack.swap(n, m)
    }

    #[cmd(name = "depth", stack)]
    fn interpret_depth(stack: &mut Stack) -> Result<()> {
        stack.push_int(stack.depth())
    }

    #[cmd(name = "?dup", stack)]
    fn interpret_cond_dup(stack: &mut Stack) -> Result<()> {
        let item = stack.pop_int()?;
        if !item.is_zero() {
            stack.push_raw(item.clone().into_dyn_fift_value())?;
        }
        stack.push_raw(item.into_dyn_fift_value())
    }

    /// === Low-level stack manipulation ===

    #[cmd(name = "<xchg>", stack)]
    fn interpret_make_xchg(stack: &mut Stack) -> Result<()> {
        let mut y = stack.pop_smallint_range(0, 255)?;
        let mut x = stack.pop_smallint_range(0, 255)?;
        if x > y {
            std::mem::swap(&mut x, &mut y);
        }

        match (x, y) {
            (0, 0) => stack.push(cont::NopCont::instance()),
            (0, 1) => stack.push(RcFiftCont::new_dyn_fift_cont(
                interpret_swap as cont::StackWordFunc,
            )),
            _ => stack.push(RcFiftCont::new_dyn_fift_cont(XchgCont { x, y })),
        }
    }

    #[cmd(name = "<push>", stack)]
    fn interpret_make_push(stack: &mut Stack) -> Result<()> {
        let x = stack.pop_smallint_range(0, 255)?;
        match x {
            0 => stack.push(RcFiftCont::new_dyn_fift_cont(
                interpret_dup as cont::StackWordFunc,
            )),
            1 => stack.push(RcFiftCont::new_dyn_fift_cont(
                interpret_over as cont::StackWordFunc,
            )),
            _ => stack.push(RcFiftCont::new_dyn_fift_cont(PushCont(x))),
        }
    }

    #[cmd(name = "<pop>", stack)]
    fn interpret_make_pop(stack: &mut Stack) -> Result<()> {
        let x = stack.pop_smallint_range(0, 255)?;
        match x {
            0 => stack.push(RcFiftCont::new_dyn_fift_cont(
                interpret_drop as cont::StackWordFunc,
            )),
            1 => stack.push(RcFiftCont::new_dyn_fift_cont(
                interpret_nip as cont::StackWordFunc,
            )),
            _ => stack.push(RcFiftCont::new_dyn_fift_cont(PopCont(x))),
        }
    }
}

struct XchgCont {
    x: u32,
    y: u32,
}

impl cont::FiftCont for XchgCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        ctx.stack.swap(self.x as usize, self.y as usize)?;
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<xchg {} {}>", self.x, self.y)
    }
}

struct PushCont(u32);

impl cont::FiftCont for PushCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let item = ctx.stack.fetch(self.0 as usize)?;
        ctx.stack.push_raw(item)?;
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<push {}>", self.0)
    }
}

struct PopCont(u32);

impl cont::FiftCont for PopCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        ctx.stack.swap(0, self.0 as usize)?;
        ctx.stack.pop()?;
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<pop {}>", self.0)
    }
}
