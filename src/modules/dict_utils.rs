use std::iter::Peekable;
use std::rc::Rc;

use anyhow::{Context as _, Result};
use everscale_types::cell::DefaultFinalizer;
use everscale_types::dict::{self, dict_get, dict_insert, dict_remove_owned, SetMode};
use everscale_types::prelude::*;
use num_bigint::BigInt;

use crate::core::cont::{LoopCont, LoopContImpl};
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
        stack.push(OwnedCellSlice::new(cell))
    }

    #[cmd(name = "dict,", stack)]
    fn interpret_store_dict(stack: &mut Stack) -> Result<()> {
        let maybe_cell = pop_maybe_cell(stack)?;
        let mut builder = stack.pop_builder()?;
        maybe_cell.store_into(Rc::make_mut(&mut builder), &mut Cell::default_finalizer())?;
        stack.push_raw(builder)
    }

    #[cmd(name = "dict@", stack, args(fetch = false))]
    #[cmd(name = "dict@+", stack, args(fetch = true))]
    fn interpret_load_dict(stack: &mut Stack, fetch: bool) -> Result<()> {
        let mut cs_raw = stack.pop_slice()?;
        let mut cs = cs_raw.apply()?;
        let cell = Option::<Cell>::load_from(&mut cs)?;
        push_maybe_cell(stack, cell)?;
        if fetch {
            let range = cs.range();
            Rc::make_mut(&mut cs_raw).set_range(range);
            stack.push_raw(cs_raw)?;
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
            key.range().remaining_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let value = if b {
            OwnedCellSlice::new(stack.pop_builder_owned()?.build()?)
        } else {
            stack.pop_slice()?.as_ref().clone()
        };
        let value = value.apply()?;

        let mut key = key.apply()?.get_prefix(bits, 0);
        let dict = dict_insert(
            &cell,
            &mut key,
            bits,
            &value,
            mode,
            &mut Cell::default_finalizer(),
        );

        // TODO: use operation result flag?
        let res = dict.is_ok();
        if let Ok((cell, _)) = dict {
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
            key.range().remaining_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let key = key.apply()?.get_prefix(bits, 0);
        let value = dict_get(&cell, bits, key).ok().flatten();

        let res = value.is_some();
        if let Some(value) = value {
            // TODO: add owned `dict_get` to remove this intermediate builder
            let mut builder = CellBuilder::new();
            builder.store_slice(value)?;
            stack.push(OwnedCellSlice::new(builder.build()?))?;
        }
        stack.push_bool(res)
    }

    #[cmd(name = "sdict@-", stack, args(key = KeyMode::Slice, ignore = false))]
    #[cmd(name = "udict@-", stack, args(key = KeyMode::Unsigned, ignore = false))]
    #[cmd(name = "idict@-", stack, args(key = KeyMode::Signed, ignore = false))]
    #[cmd(name = "sdict-", stack, args(key = KeyMode::Slice, ignore = true))]
    #[cmd(name = "udict-", stack, args(key = KeyMode::Unsigned, ignore = true))]
    #[cmd(name = "idict-", stack, args(key = KeyMode::Signed, ignore = true))]
    fn interpret_dict_remove(stack: &mut Stack, key: KeyMode, ignore: bool) -> Result<()> {
        let bits = stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let cell = pop_maybe_cell(stack)?;
        let key = pop_dict_key(stack, key, bits)?;
        anyhow::ensure!(
            key.range().remaining_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let key = &mut key.apply()?.get_prefix(bits, 0);
        let value = dict_remove_owned(&cell, key, bits, false, &mut Cell::default_finalizer()).ok();

        let (dict, value) = match value {
            Some((dict, value)) => (dict, value),
            None => (cell.clone(), None),
        };

        push_maybe_cell(stack, dict)?;

        let found = value.is_some();
        if !ignore {
            if let Some(value) = value {
                stack.push(OwnedCellSlice::from(value))?;
            }
        }
        stack.push_bool(found)
    }

    #[cmd(name = "dictmap", tail, args(ext = false, s = false))]
    #[cmd(name = "dictmapext", tail, args(ext = true, s = false))]
    #[cmd(name = "idictmapext", tail, args(ext = true, s = true))]
    fn interpret_dict_map(ctx: &mut Context, ext: bool, s: bool) -> Result<Option<Cont>> {
        let func = ctx.stack.pop_cont()?.as_ref().clone();
        let bits = ctx.stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let cell = pop_maybe_cell(&mut ctx.stack)?;
        Ok(Some(Rc::new(LoopCont::new(
            DictMapCont {
                iter: OwnedDictIter::new(cell, bits, false, s).peekable(),
                pos: None,
                extended: ext,
                signed: s,
                result: None,
            },
            func,
            ctx.next.take(),
        ))))
    }

    #[cmd(name = "dictforeach", tail, args(r = false, s = false))]
    #[cmd(name = "idictforeach", tail, args(r = false, s = true))]
    #[cmd(name = "dictforeachrev", tail, args(r = true, s = false))]
    #[cmd(name = "idictforeachrev", tail, args(r = true, s = true))]
    fn interpret_dict_foreach(ctx: &mut Context, r: bool, s: bool) -> Result<Option<Cont>> {
        let func = ctx.stack.pop_cont()?.as_ref().clone();
        let bits = ctx.stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let cell = pop_maybe_cell(&mut ctx.stack)?;
        Ok(Some(Rc::new(LoopCont::new(
            DictIterCont {
                iter: OwnedDictIter::new(cell, bits, r, s).peekable(),
                signed: s,
                ok: true,
            },
            func,
            ctx.next.take(),
        ))))
    }
}

#[derive(Clone)]
struct DictMapCont {
    iter: Peekable<OwnedDictIter>,
    pos: Option<CellBuilder>,
    result: Option<Cell>,
    extended: bool,
    signed: bool,
}

impl LoopContImpl for DictMapCont {
    fn pre_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        let (key, value) = match self.iter.next() {
            Some(entry) => entry?,
            None => return Ok(false),
        };
        ctx.stack.push(CellBuilder::new())?;
        if self.extended {
            ctx.stack.push(builder_to_int(&key, self.signed)?)?;
        }
        ctx.stack.push(value)?;
        self.pos = Some(key);
        Ok(true)
    }

    fn post_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        if ctx.stack.pop_bool()? {
            let key = self
                .pos
                .as_ref()
                .context("Uninitialized dictmap iterator")?;

            let value = ctx.stack.pop_builder()?;
            let (new_root, _) = dict_insert(
                &self.result,
                &mut key.as_data_slice(),
                key.bit_len(),
                &value.as_full_slice(),
                SetMode::Set,
                &mut Cell::default_finalizer(),
            )?;
            self.result = new_root;
        }

        Ok(self.iter.peek().is_some())
    }

    fn finalize(&mut self, ctx: &mut Context) -> Result<bool> {
        push_maybe_cell(&mut ctx.stack, self.result.take())?;
        Ok(true)
    }
}

#[derive(Clone)]
struct DictIterCont {
    iter: Peekable<OwnedDictIter>,
    signed: bool,
    ok: bool,
}

impl LoopContImpl for DictIterCont {
    fn pre_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        let (key, value) = match self.iter.next() {
            Some(entry) => entry?,
            None => return Ok(false),
        };

        ctx.stack.push(builder_to_int(&key, self.signed)?)?;
        ctx.stack.push(value)?;
        Ok(true)
    }

    fn post_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        self.ok = ctx.stack.pop_bool()?;
        Ok(self.ok && self.iter.peek().is_some())
    }

    fn finalize(&mut self, ctx: &mut Context) -> Result<bool> {
        ctx.stack.push_bool(self.ok)?;
        Ok(true)
    }
}

#[derive(Clone)]
struct OwnedDictIter {
    root: Option<Cell>,
    inner: dict::RawIter<'static>,
}

impl OwnedDictIter {
    fn new(root: Option<Cell>, bit_len: u16, reversed: bool, signed: bool) -> Self {
        let inner = dict::RawIter::new_ext(&root, bit_len, reversed, signed);

        // SAFETY: iter lifetime is bounded to the `DynCell` which lives as long
        // as `Cell` lives. By storing root we guarantee that it will live enough.
        let inner = unsafe { std::mem::transmute::<_, dict::RawIter<'static>>(inner) };

        Self { root, inner }
    }
}

impl Iterator for OwnedDictIter {
    type Item = Result<(CellBuilder, OwnedCellSlice), everscale_types::error::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next_owned(&self.root)? {
            Ok((key, value)) => Ok((key, OwnedCellSlice::from(value))),
            Err(e) => Err(e),
        })
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
        Some(value.into_cell()?.as_ref().clone())
    })
}

fn pop_dict_key(stack: &mut Stack, key_mode: KeyMode, bits: u16) -> Result<OwnedCellSlice> {
    let signed = match key_mode {
        KeyMode::Slice => return Ok(stack.pop_slice()?.as_ref().clone()),
        KeyMode::Signed => true,
        KeyMode::Unsigned => false,
    };

    let mut builder = CellBuilder::new();
    let int = stack.pop_int()?;
    store_int_to_builder(&mut builder, &int, bits, signed)?;
    Ok(OwnedCellSlice::new(builder.build()?))
}

fn builder_to_int(builder: &CellBuilder, signed: bool) -> Result<BigInt> {
    let bits = builder.bit_len();
    anyhow::ensure!(
        bits <= (256 + signed as u16),
        "Key does not fit into integer"
    );

    let bytes = ((bits + 7) / 8) as usize;
    let mut int = BigInt::from_signed_bytes_be(&builder.raw_data()[..bytes]);

    let rem = bits % 8;
    if rem != 0 {
        int >>= 8 - rem;
    }
    Ok(int)
}

const MAX_KEY_BITS: u32 = 1023;
