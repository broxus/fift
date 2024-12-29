use anyhow::Result;
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, Zero};
use tycho_types::prelude::*;
use tycho_vm::SafeRc;

use crate::core::*;

pub struct Arithmetic;

#[fift_module]
impl Arithmetic {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        let mut make_int_lit = |name: &str, value: i32| {
            d.define_word(name, SafeRc::new(cont::IntLitCont::from(value)))
        };

        make_int_lit("false ", 0)?;
        make_int_lit("true ", -1)?;
        make_int_lit("0 ", 0)?;
        make_int_lit("1 ", 1)?;
        make_int_lit("2 ", 2)?;
        make_int_lit("-1 ", -1)?;
        make_int_lit("bl ", 32)
    }

    // === Basic ===

    #[cmd(name = "+", stack)]
    fn interpret_plus(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) += y.as_ref();
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "-", stack)]
    fn interpret_minus(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) -= y.as_ref();
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "1+", stack, args(rhs = 1))]
    #[cmd(name = "1-", stack, args(rhs = -1))]
    #[cmd(name = "2+", stack, args(rhs = 2))]
    #[cmd(name = "2-", stack, args(rhs = -2))]
    fn interpret_plus_const(stack: &mut Stack, rhs: i32) -> Result<()> {
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) += rhs;
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "negate", stack)]
    fn interpret_negate(stack: &mut Stack) -> Result<()> {
        let mut x = stack.pop_int()?;
        {
            let x = SafeRc::make_mut(&mut x);
            *x = -std::mem::take(x);
        }
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "*", stack)]
    fn interpret_mul(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) *= y.as_ref();
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "/", stack, args(r = Rounding::Floor))]
    #[cmd(name = "/r", stack, args(r = Rounding::Nearest))]
    #[cmd(name = "/c", stack, args(r = Rounding::Ceil))]
    fn interpret_div(stack: &mut Stack, r: Rounding) -> Result<()> {
        let y = stack.pop_int()?;
        let x = stack.pop_int()?;
        stack.push(divmod(&x, &y, r)?.0)
    }

    #[cmd(name = "mod", stack, args(r = Rounding::Floor))]
    #[cmd(name = "rmod", stack, args(r = Rounding::Nearest))]
    #[cmd(name = "cmod", stack, args(r = Rounding::Ceil))]
    fn interpret_mod(stack: &mut Stack, r: Rounding) -> Result<()> {
        let y = stack.pop_int()?;
        let x = stack.pop_int()?;
        stack.push(divmod(&x, &y, r)?.1)
    }

    #[cmd(name = "/mod", stack, args(r = Rounding::Floor))]
    #[cmd(name = "/rmod", stack, args(r = Rounding::Nearest))]
    #[cmd(name = "/cmod", stack, args(r = Rounding::Ceil))]
    fn interpret_divmod(stack: &mut Stack, r: Rounding) -> Result<()> {
        let y = stack.pop_int()?;
        let x = stack.pop_int()?;
        let (q, r) = divmod(&x, &y, r)?;
        stack.push(q)?;
        stack.push(r)
    }

    #[cmd(name = "*/", stack, args(r = Rounding::Floor))]
    #[cmd(name = "*/r", stack, args(r = Rounding::Nearest))]
    #[cmd(name = "*/c", stack, args(r = Rounding::Ceil))]
    fn interpret_times_div(stack: &mut Stack, r: Rounding) -> Result<()> {
        let z = stack.pop_int()?;
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) *= y.as_ref();
        stack.push(divmod(&x, &z, r)?.0)
    }

    #[cmd(name = "*/mod", stack, args(r = Rounding::Floor))]
    #[cmd(name = "*/rmod", stack, args(r = Rounding::Nearest))]
    #[cmd(name = "*/cmod", stack, args(r = Rounding::Ceil))]
    fn interpret_times_divmod(stack: &mut Stack, r: Rounding) -> Result<()> {
        let z = stack.pop_int()?;
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) *= y.as_ref();
        let (q, r) = divmod(&x, &z, r)?;
        stack.push(q)?;
        stack.push(r)
    }

    #[cmd(name = "*mod", stack, args(r = Rounding::Floor))]
    fn interpret_times_mod(stack: &mut Stack, r: Rounding) -> Result<()> {
        let z = stack.pop_int()?;
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) *= y.as_ref();
        stack.push(divmod(&x, &z, r)?.1)
    }

    #[cmd(name = "1<<", stack, args(negate = false, minus_one = false))]
    #[cmd(name = "-1<<", stack, args(negate = true, minus_one = false))]
    #[cmd(name = "1<<1-", stack, args(negate = false, minus_one = true))]
    fn interpret_pow2(stack: &mut Stack, negate: bool, minus_one: bool) -> Result<()> {
        let x = stack.pop_smallint_range(0, 255 + (negate || minus_one) as u32)? as u16;
        let mut res = BigInt::one();
        res <<= x;
        if minus_one {
            res -= 1;
        }
        if negate {
            res = -res;
        }
        stack.push(res)
    }

    #[cmd(name = "%1<<", stack)]
    fn interpret_mod_pow2(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_smallint_range(0, 256)? as u16;
        let mut x = stack.pop_int()?;
        let mut mask = BigInt::one();
        mask <<= y;
        mask -= 1;
        *SafeRc::make_mut(&mut x) &= mask;
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "<<", stack)]
    fn interpret_lshift(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_smallint_range(0, 256)? as u16;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) <<= y;
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = ">>", stack, args(r = Rounding::Floor))]
    #[cmd(name = ">>r", stack, args(r = Rounding::Nearest))]
    #[cmd(name = ">>c", stack, args(r = Rounding::Ceil))]
    fn interpret_rshift(stack: &mut Stack, r: Rounding) -> Result<()> {
        let y = stack.pop_smallint_range(0, 256)? as u16;
        let mut x = stack.pop_int()?;
        match r {
            Rounding::Floor => *SafeRc::make_mut(&mut x) >>= y,
            // TODO
            _ => anyhow::bail!("Unimplemented"),
        }
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "2*", stack, args(y = 1))]
    fn interpret_lshift_const(stack: &mut Stack, y: u8) -> Result<()> {
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) <<= y;
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "2/", stack, args(y = 1))]
    fn interpret_rshift_const(stack: &mut Stack, y: u8) -> Result<()> {
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) >>= y;
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "<</", stack, args(r = Rounding::Floor))]
    #[cmd(name = "<</r", stack, args(r = Rounding::Nearest))]
    #[cmd(name = "<</c", stack, args(r = Rounding::Ceil))]
    fn interpret_lshift_div(stack: &mut Stack, r: Rounding) -> Result<()> {
        let z = stack.pop_smallint_range(0, 256)?;
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) <<= z;
        stack.push(divmod(&x, &y, r)?.0)
    }

    // TODO: mul shift, shift div

    // === Logical ===

    #[cmd(name = "not", stack)]
    fn interpret_not(stack: &mut Stack) -> Result<()> {
        let mut x = stack.pop_int()?;
        {
            let lhs = SafeRc::make_mut(&mut x);
            *lhs = !std::mem::take(lhs);
        }
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "and", stack)]
    fn interpret_and(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) &= y.as_ref();
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "or", stack)]
    fn interpret_or(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) |= y.as_ref();
        stack.push_raw(x.into_dyn_fift_value())
    }

    #[cmd(name = "xor", stack)]
    fn interpret_xor(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_int()?;
        let mut x = stack.pop_int()?;
        *SafeRc::make_mut(&mut x) ^= y.as_ref();
        stack.push_raw(x.into_dyn_fift_value())
    }

    // === Integer comparison ===

    #[cmd(name = "cmp", stack, args(map = [-1, 0, 1]))]
    #[cmd(name = "=", stack, args(map = [0, -1, 0]))]
    #[cmd(name = "<>", stack, args(map = [-1, 0, -1]))]
    #[cmd(name = "<=", stack, args(map = [-1, -1, 0]))]
    #[cmd(name = ">=", stack, args(map = [0, -1, -1]))]
    #[cmd(name = "<", stack, args(map = [-1, 0, 0]))]
    #[cmd(name = ">", stack, args(map = [0, 0, -1]))]
    fn interpret_cmp(stack: &mut Stack, map: [i8; 3]) -> Result<()> {
        let y = stack.pop_int()?;
        let x = stack.pop_int()?;
        let map_index = x.cmp(&y) as i8 + 1;
        stack.push_int(map[map_index as usize])
    }

    #[cmd(name = "sgn", stack, args(map = [-1, 0, 1]))]
    #[cmd(name = "0=", stack, args(map = [0, -1, 0]))]
    #[cmd(name = "0<>", stack, args(map = [-1, 0, -1]))]
    #[cmd(name = "0<=", stack, args(map = [-1, -1, 0]))]
    #[cmd(name = "0>=", stack, args(map = [0, -1, -1]))]
    #[cmd(name = "0<", stack, args(map = [-1, 0, 0]))]
    #[cmd(name = "0>", stack, args(map = [0, 0, -1]))]
    fn interpret_sgn(stack: &mut Stack, map: [i8; 3]) -> Result<()> {
        let x = stack.pop_int()?;
        let map_index = match x.sign() {
            Sign::Minus => 0,
            Sign::NoSign => 1,
            Sign::Plus => 2,
        };
        stack.push_int(map[map_index as usize])
    }

    #[cmd(name = "fits", stack, args(signed = true))]
    #[cmd(name = "ufits", stack, args(signed = false))]
    fn interpret_fits(stack: &mut Stack, signed: bool) -> Result<()> {
        let y = stack.pop_smallint_range(0, 1023)? as u16;
        let x = stack.pop_int()?;
        let bits = x.bitsize(signed);
        stack.push_bool(bits <= y)
    }
}

enum Rounding {
    Floor,
    Nearest,
    Ceil,
}

// Math code from:
// https://github.com/tonlabs/ever-vm/blob/master/src/stack/integer/math.rs

#[inline]
fn divmod(lhs: &BigInt, rhs: &BigInt, rounding: Rounding) -> Result<(BigInt, BigInt)> {
    anyhow::ensure!(!rhs.is_zero(), "Division by zero");
    Ok(match rounding {
        Rounding::Floor => lhs.div_mod_floor(rhs),
        Rounding::Nearest => {
            let (mut q, mut r) = lhs.div_rem(rhs);
            round_nearest(&mut q, &mut r, lhs, rhs);
            (q, r)
        }
        Rounding::Ceil => {
            let (mut q, mut r) = lhs.div_rem(rhs);
            round_ceil(&mut q, &mut r, lhs, rhs);
            (q, r)
        }
    })
}

#[inline]
fn round_ceil(q: &mut BigInt, r: &mut BigInt, lhs: &BigInt, rhs: &BigInt) {
    if r.is_zero() || r.sign() != rhs.sign() {
        return;
    }
    *r -= rhs;
    if lhs.sign() == rhs.sign() {
        *q += 1;
    } else {
        *q -= 1;
    }
}

#[inline]
fn round_nearest(q: &mut BigInt, r: &mut BigInt, lhs: &BigInt, rhs: &BigInt) {
    if r.is_zero() {
        return;
    }
    //  5 / 2  ->   2,  1  ->   3, -1
    // -5 / 2  ->  -2, -1  ->  -2, -1
    //  5 /-2  ->  -2,  1  ->  -2,  1
    // -5 /-2  ->   2, -1  ->   3,  1
    let r_x2: BigInt = r.clone() << 1;
    let cmp_result = r_x2.abs().cmp(&rhs.abs());
    let is_not_negative = lhs.sign() == rhs.sign();
    if cmp_result == std::cmp::Ordering::Greater
        || (cmp_result == std::cmp::Ordering::Equal && is_not_negative)
    {
        if rhs.sign() == r.sign() {
            *r -= rhs;
        } else {
            *r += rhs;
        }
        if is_not_negative {
            *q += 1;
        } else {
            *q -= 1;
        }
    }
}
