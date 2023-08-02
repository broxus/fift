use anyhow::Result;
use everscale_types::cell::DefaultFinalizer;
use everscale_types::dict::*;
use everscale_types::prelude::*;

use crate::core::*;
use crate::util::*;

pub struct DictUtils;

#[fift_module]
impl DictUtils {
    #[cmd(name = "dictnew", stack)]
    fn interpret_dict_new(stack: &mut Stack) -> Result<()> {
        stack.push(())
    }

    #[cmd(name = "dict>s", stack)]
    fn interpret_dict_to_slice(stack: &mut Stack) -> Result<()> {
        let maybe_cell = pop_maybe_cell(stack)?;
        let cell = CellBuilder::build_from(maybe_cell)?;
        stack.push(OwnedCellSlice::new(cell)?)
    }

    #[cmd(name = "dict,", stack)]
    fn interpret_store_dict(stack: &mut Stack) -> Result<()> {
        let maybe_cell = pop_maybe_cell(stack)?;
        let mut builder = stack.pop_builder()?;
        maybe_cell.store_into(&mut builder, &mut Cell::default_finalizer())?;
        stack.push_raw(builder)
    }

    #[cmd(name = "dict@", stack, args(fetch = false))]
    #[cmd(name = "dict@+", stack, args(fetch = true))]
    fn interpret_load_dict(stack: &mut Stack, fetch: bool) -> Result<()> {
        let mut cs = stack.pop_slice()?;
        let cell = Option::<Cell>::load_from(cs.pin_mut())?;
        push_maybe_cell(stack, cell)?;
        if fetch {
            stack.push_raw(cs)?;
        }
        Ok(())
    }

    // Slice
    #[cmd(name = "sdict!+", stack, args(b = false, mode = SetMode::Add, key = KeyMode::Slice))]
    #[cmd(name = "sdict!", stack, args(b = false, mode = SetMode::Set, key = KeyMode::Slice))]
    #[cmd(name = "b>sdict!+", stack, args(b = true, mode = SetMode::Add, key = KeyMode::Slice))]
    #[cmd(name = "b>sdict!", stack, args(b = true, mode = SetMode::Set, key = KeyMode::Slice))]
    // Unsigned
    #[cmd(name = "udict!+", stack, args(b = false, mode = SetMode::Add, key = KeyMode::Unsigned))]
    #[cmd(name = "udict!", stack, args(b = false, mode = SetMode::Set, key = KeyMode::Unsigned))]
    #[cmd(name = "b>udict!+", stack, args(b = true, mode = SetMode::Add, key = KeyMode::Unsigned))]
    #[cmd(name = "b>udict!", stack, args(b = true, mode = SetMode::Set, key = KeyMode::Unsigned))]
    // Signed
    #[cmd(name = "idict!+", stack, args(b = false, mode = SetMode::Add, key = KeyMode::Signed))]
    #[cmd(name = "idict!", stack, args(b = false, mode = SetMode::Set, key = KeyMode::Signed))]
    #[cmd(name = "b>idict!+", stack, args(b = true, mode = SetMode::Add, key = KeyMode::Signed))]
    #[cmd(name = "b>idict!", stack, args(b = true, mode = SetMode::Set, key = KeyMode::Signed))]
    fn interpret_dict_add(stack: &mut Stack, b: bool, mode: SetMode, key: KeyMode) -> Result<()> {
        let bits = stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let cell = pop_maybe_cell(stack)?;
        let key = pop_dict_key(stack, key, bits)?;
        anyhow::ensure!(
            key.pin().remaining_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let value = if b {
            OwnedCellSlice::new(stack.pop_builder()?.build()?)?
        } else {
            *stack.pop_slice()?
        };
        let value = value.pin();

        let mut key = key.pin().get_prefix(bits, 0);
        let dict = dict_insert(
            &cell,
            &mut key,
            bits,
            value,
            mode,
            &mut Cell::default_finalizer(),
        );

        let res = dict.is_ok();
        if let Ok(cell) = dict {
            push_maybe_cell(stack, cell)?;
        }
        stack.push_bool(res)
    }

    #[cmd(name = "sdict@", stack, args(key = KeyMode::Slice))]
    #[cmd(name = "udict@", stack, args(key = KeyMode::Unsigned))]
    #[cmd(name = "idict@", stack, args(key = KeyMode::Signed))]
    fn interpret_dict_get(stack: &mut Stack, key: KeyMode) -> Result<()> {
        let bits = stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let cell = pop_maybe_cell(stack)?;
        let key = pop_dict_key(stack, key, bits)?;
        anyhow::ensure!(
            key.pin().remaining_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let key = key.pin().get_prefix(bits, 0);
        let value = dict_get(&cell, bits, key).ok().flatten();

        let res = value.is_some();
        if let Some(value) = value {
            // TODO: add owned `dict_get` to remove this intermediate builder
            let mut builder = CellBuilder::new();
            builder.store_slice(value)?;
            stack.push(OwnedCellSlice::new(builder.build()?)?)?;
        }
        stack.push_bool(res)
    }
}

enum KeyMode {
    Slice,
    Unsigned,
    Signed,
}

fn push_maybe_cell(stack: &mut Stack, cell: Option<Cell>) -> Result<()> {
    match cell {
        Some(cell) => stack.push(cell),
        None => stack.push(()),
    }
}

fn pop_maybe_cell(stack: &mut Stack) -> Result<Option<Cell>> {
    let value = stack.pop()?;
    Ok(if value.is_null() {
        None
    } else {
        Some(*value.into_cell()?)
    })
}

fn pop_dict_key(stack: &mut Stack, key_mode: KeyMode, bits: u16) -> Result<OwnedCellSlice> {
    let signed = match key_mode {
        KeyMode::Slice => return Ok(*stack.pop_slice()?),
        KeyMode::Signed => true,
        KeyMode::Unsigned => false,
    };

    let mut builder = CellBuilder::new();
    let mut int = stack.pop_int()?;
    store_int_to_builder(&mut builder, &mut int, bits, signed)?;
    OwnedCellSlice::new(builder.build()?).map_err(From::from)
}

const MAX_KEY_BITS: u32 = 1023;
