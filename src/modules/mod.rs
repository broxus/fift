use crate::core::*;
use crate::error::*;

pub use self::arithmetic::Arithmetic;
pub use self::cell_utils::CellUtils;
pub use self::control::Control;
pub use self::crypto::Crypto;
pub use self::debug_utils::DebugUtils;
pub use self::stack_utils::StackUtils;
pub use self::string_utils::StringUtils;

mod arithmetic;
mod cell_utils;
mod control;
mod crypto;
mod debug_utils;
mod stack_utils;
mod string_utils;

pub struct BaseModule;

#[fift_module]
impl FiftModule for BaseModule {
    #[init]
    fn init(d: &mut Dictionary) -> Result<()> {
        d.define_word("nop", DictionaryEntry::new_ordinary(d.make_nop()), false)
    }

    #[cmd(name = "null", stack)]
    fn interpret_push_null(stack: &mut Stack) -> Result<()> {
        stack.push(())
    }

    #[cmd(name = "null?", stack, args(ty = StackValueType::Null))]
    #[cmd(name = "integer?", stack, args(ty = StackValueType::Int))]
    #[cmd(name = "string?", stack, args(ty = StackValueType::String))]
    #[cmd(name = "tuple?", stack, args(ty = StackValueType::Tuple))]
    fn interpret_is_type(stack: &mut Stack, ty: StackValueType) -> Result<()> {
        let is_ty = stack.pop()?.ty() == ty;
        stack.push_bool(is_ty)
    }

    #[cmd(name = "hole", stack)]
    fn interpret_hole(stack: &mut Stack) -> Result<()> {
        stack.push(SharedBox::default())
    }

    #[cmd(name = "box", stack)]
    fn interpret_box(stack: &mut Stack) -> Result<()> {
        let value = stack.pop()?;
        stack.push(SharedBox::new(value))
    }

    #[cmd(name = "@", stack)]
    fn interpret_box_fetch(stack: &mut Stack) -> Result<()> {
        let value = stack.pop_shared_box()?;
        stack.push_raw(value.fetch())
    }

    #[cmd(name = "!", stack)]
    fn interpret_box_store(stack: &mut Stack) -> Result<()> {
        let value = stack.pop_shared_box()?;
        value.store(stack.pop()?);
        Ok(())
    }

    #[cmd(name = "|", stack)]
    fn interpret_empty_tuple(stack: &mut Stack) -> Result<()> {
        stack.push(StackTuple::new())
    }

    #[cmd(name = ",", stack)]
    fn interpret_tuple_push(stack: &mut Stack) -> Result<()> {
        let value = stack.pop()?;
        let mut tuple = stack.pop_tuple()?;
        tuple.push(value);
        stack.push_raw(tuple)
    }

    #[cmd(name = "tpop", stack)]
    fn interpret_tuple_pop(stack: &mut Stack) -> Result<()> {
        let mut tuple = stack.pop_tuple()?;
        let last = tuple.pop().ok_or(Error::TupleUnderflow)?;
        stack.push_raw(tuple)?;
        stack.push_raw(last)
    }

    #[cmd(name = "[]", stack)]
    fn interpret_tuple_index(stack: &mut Stack) -> Result<()> {
        let idx = stack.pop_usize()?;
        let tuple = stack.pop_tuple()?;
        let value = tuple.get(idx).ok_or(Error::IndexOutOfRange)?;
        stack.push_raw(dyn_clone::clone_box(value.as_ref()))
    }

    #[cmd(name = "[]=", stack)]
    fn interpret_tuple_set(stack: &mut Stack) -> Result<()> {
        let idx = stack.pop_usize()?;
        let value = stack.pop()?;
        let mut tuple = stack.pop_tuple()?;
        *tuple.get_mut(idx).ok_or(Error::IndexOutOfRange)? = value;
        stack.push_raw(tuple)
    }

    #[cmd(name = "count", stack)]
    fn interpret_tuple_len(stack: &mut Stack) -> Result<()> {
        let len = stack.pop_tuple()?.len();
        stack.push_int(len)
    }

    #[cmd(name = "tuple", stack)]
    fn interpret_make_tuple(stack: &mut Stack) -> Result<()> {
        let n = stack.pop_smallint_range(0, 255)? as usize;
        let mut tuple = Vec::with_capacity(n);
        for _ in 0..n {
            tuple.push(stack.pop()?);
        }
        tuple.reverse();
        stack.push(tuple)
    }

    #[cmd(name = "untuple", stack, args(pop_count = true))]
    #[cmd(name = "explode", stack, args(pop_count = false))]
    fn interpret_tuple_explode(stack: &mut Stack, pop_count: bool) -> Result<()> {
        let mut n = if pop_count {
            stack.pop_smallint_range(0, 255)? as usize
        } else {
            0
        };
        let tuple = stack.pop_tuple()?;
        if !pop_count {
            n = tuple.len();
            if n > 255 {
                return Err(Error::TupleTooLarge);
            }
        } else if tuple.len() != n {
            return Err(Error::TupleSizeMismatch);
        }

        for item in *tuple {
            stack.push_raw(item)?;
        }

        if !pop_count {
            stack.push_int(n)?;
        }

        Ok(())
    }

    #[cmd(name = "now")]
    fn interpret_now(ctx: &mut Context) -> Result<()> {
        ctx.stack.push_int(ctx.env.now_ms() / 1000)
    }

    #[cmd(name = "now_ms")]
    fn interpret_now_ms(ctx: &mut Context) -> Result<()> {
        ctx.stack.push_int(ctx.env.now_ms())
    }

    #[cmd(name = "getenv")]
    fn interpret_getenv(ctx: &mut Context) -> Result<()> {
        let name = ctx.stack.pop_string()?;
        let value = ctx.env.get_env(&name).unwrap_or_default();
        ctx.stack.push(value)
    }

    #[cmd(name = "getenv?")]
    fn interpret_getenv_exists(ctx: &mut Context) -> Result<()> {
        let name = ctx.stack.pop_string()?;
        let exists = match ctx.env.get_env(&name) {
            Some(value) => {
                ctx.stack.push(value)?;
                true
            }
            None => false,
        };
        ctx.stack.push_bool(exists)
    }
}
