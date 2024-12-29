use std::iter::Peekable;
use std::rc::Rc;

use anyhow::{Context as _, Result};
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
        stack.push_null()
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
        maybe_cell.store_into(Rc::make_mut(&mut builder), &mut Cell::empty_context())?;
        stack.push_raw(builder)
    }

    #[cmd(name = "dict@", stack, args(fetch = false))]
    #[cmd(name = "dict@+", stack, args(fetch = true))]
    fn interpret_load_dict(stack: &mut Stack, fetch: bool) -> Result<()> {
        let mut cs_raw = stack.pop_slice()?;
        let mut cs = cs_raw.apply()?;
        let cell = Option::<Cell>::load_from(&mut cs)?;
        stack.push_opt(cell)?;
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
        let mut cell = pop_maybe_cell(stack)?;
        let key = pop_dict_key(stack, key, bits)?;
        anyhow::ensure!(
            key.range().size_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let value = if b {
            OwnedCellSlice::new(stack.pop_builder_owned()?.build()?)
        } else {
            stack.pop_slice()?.as_ref().clone()
        };
        let value = value.apply()?;

        let mut key = key.apply()?.get_prefix(bits, 0);
        let res = dict_insert(
            &mut cell,
            &mut key,
            bits,
            &value,
            mode,
            &mut Cell::empty_context(),
        );

        // TODO: use operation result flag?
        let res = res.is_ok();
        if res {
            stack.push_opt(cell)?;
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
            key.range().size_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let key = key.apply()?.get_prefix(bits, 0);
        let value = dict_get(cell.as_ref(), bits, key, &mut Cell::empty_context())
            .ok()
            .flatten();

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
        let mut dict = pop_maybe_cell(stack)?;
        let key = pop_dict_key(stack, key, bits)?;
        anyhow::ensure!(
            key.range().size_bits() >= bits,
            "Not enough bits for a dictionary key"
        );

        let key = &mut key.apply()?.get_prefix(bits, 0);
        let value = dict_remove_owned(&mut dict, key, bits, false, &mut Cell::empty_context())
            .ok()
            .flatten();

        stack.push_opt(dict)?;

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
        let func = ctx.stack.pop_cont_owned()?;
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
        let func = ctx.stack.pop_cont_owned()?;
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

    #[cmd(name = "dictmerge", tail)]
    fn interpret_dict_merge(ctx: &mut Context) -> Result<Option<Cont>> {
        let func = ctx.stack.pop_cont_owned()?;
        let bits = ctx.stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let right = pop_maybe_cell(&mut ctx.stack)?;
        let left = pop_maybe_cell(&mut ctx.stack)?;
        Ok(Some(Rc::new(LoopCont::new(
            DictMergeCont {
                left: OwnedDictIter::new(left, bits, false, false).peekable(),
                right: OwnedDictIter::new(right, bits, false, false).peekable(),
                pos: None,
                result: None,
            },
            func,
            ctx.next.take(),
        ))))
    }

    #[cmd(name = "dictdiff", tail)]
    fn interpret_dict_diff(ctx: &mut Context) -> Result<Option<Cont>> {
        let func = ctx.stack.pop_cont_owned()?;
        let bits = ctx.stack.pop_smallint_range(0, MAX_KEY_BITS)? as u16;
        let right = pop_maybe_cell(&mut ctx.stack)?;
        let left = pop_maybe_cell(&mut ctx.stack)?;
        Ok(Some(Rc::new(LoopCont::new(
            DictDiffCont {
                left: OwnedDictIter::new(left, bits, false, false).peekable(),
                right: OwnedDictIter::new(right, bits, false, false).peekable(),
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
            dict_insert(
                &mut self.result,
                &mut key.as_data_slice(),
                key.size_bits(),
                &value.as_full_slice(),
                SetMode::Set,
                &mut Cell::empty_context(),
            )?;
        }

        Ok(self.iter.peek().is_some())
    }

    fn finalize(&mut self, ctx: &mut Context) -> Result<bool> {
        ctx.stack.push_opt(self.result.take())?;
        Ok(true)
    }
}

#[derive(Clone)]
struct DictDiffCont {
    left: Peekable<OwnedDictIter>,
    right: Peekable<OwnedDictIter>,
    ok: bool,
}

impl LoopContImpl for DictDiffCont {
    fn pre_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        Ok(loop {
            let left = self.left.peek().map(clone_error).transpose()?;
            let right = self.right.peek().map(clone_error).transpose()?;
            let (iter, swap) = match (left, right) {
                (None, None) => break false,
                (Some(_), None) => (&mut self.left, false),
                (None, Some(_)) => (&mut self.right, true),
                (Some((left_key, _)), Some((right_key, _))) => match left_key.cmp(right_key) {
                    std::cmp::Ordering::Less => (&mut self.left, false),
                    std::cmp::Ordering::Greater => (&mut self.right, true),
                    std::cmp::Ordering::Equal => {
                        let (key, left_value) = self.left.next().unwrap()?;
                        let (_, right_value) = self.right.next().unwrap()?;

                        if left_value.apply()?.lex_cmp(&right_value.apply()?)?
                            == std::cmp::Ordering::Equal
                        {
                            continue;
                        }

                        ctx.stack.push(builder_to_int(&key, false)?)?;
                        ctx.stack.push(left_value)?;
                        ctx.stack.push(right_value)?;
                        break true;
                    }
                },
            };

            let (key, value) = iter.next().unwrap()?;
            ctx.stack.push(builder_to_int(&key, false)?)?;
            if !swap {
                ctx.stack.push(value)?;
                ctx.stack.push_null()?;
            } else {
                ctx.stack.push_null()?;
                ctx.stack.push(value)?;
            }
            break true;
        })
    }

    fn post_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        self.ok = ctx.stack.pop_bool()?;
        Ok(self.ok)
    }

    fn finalize(&mut self, ctx: &mut Context) -> Result<bool> {
        ctx.stack.push_bool(self.ok)?;
        Ok(true)
    }
}

#[derive(Clone)]
struct DictMergeCont {
    left: Peekable<OwnedDictIter>,
    right: Peekable<OwnedDictIter>,
    pos: Option<CellBuilder>,
    result: Option<Cell>,
}

impl LoopContImpl for DictMergeCont {
    fn pre_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        fn clone_error(
            res: &<OwnedDictIter as Iterator>::Item,
        ) -> Result<&(CellBuilder, OwnedCellSlice)> {
            match res {
                Ok(value) => Ok(value),
                Err(e) => Err(e.clone().into()),
            }
        }

        let (left_iter, right_iter) = loop {
            let left = self.left.peek().map(clone_error).transpose()?;
            let right = self.right.peek().map(clone_error).transpose()?;
            let iter = match (left, right) {
                (None, None) => return Ok(false),
                (Some(_), None) => &mut self.left,
                (None, Some(_)) => &mut self.right,
                (Some((left_key, _)), Some((right_key, _))) => match left_key.cmp(right_key) {
                    std::cmp::Ordering::Less => &mut self.left,
                    std::cmp::Ordering::Equal => break (&mut self.left, &mut self.right),
                    std::cmp::Ordering::Greater => &mut self.right,
                },
            };
            let (key, value) = iter.next().unwrap()?;
            dict_insert(
                &mut self.result,
                &mut key.as_data_slice(),
                key.size_bits(),
                &value.apply()?,
                SetMode::Set,
                &mut Cell::empty_context(),
            )?;
        };

        let (key, left) = left_iter.next().unwrap()?;
        let (_, right) = right_iter.next().unwrap()?;

        ctx.stack.push(CellBuilder::new())?;
        ctx.stack.push(left)?;
        ctx.stack.push(right)?;
        self.pos = Some(key);
        Ok(true)
    }

    fn post_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        if ctx.stack.pop_bool()? {
            let key = self
                .pos
                .as_ref()
                .context("Uninitialized dictmerge iterator")?;

            let value = ctx.stack.pop_builder()?;
            dict_insert(
                &mut self.result,
                &mut key.as_data_slice(),
                key.size_bits(),
                &value.as_full_slice(),
                SetMode::Set,
                &mut Cell::empty_context(),
            )?;
        }

        Ok(true)
    }

    fn finalize(&mut self, ctx: &mut Context) -> Result<bool> {
        ctx.stack.push_opt(self.result.take())?;
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
        let inner =
            unsafe { std::mem::transmute::<dict::RawIter<'_>, dict::RawIter<'static>>(inner) };

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

fn clone_error(res: &<OwnedDictIter as Iterator>::Item) -> Result<&(CellBuilder, OwnedCellSlice)> {
    match res {
        Ok(value) => Ok(value),
        Err(e) => Err(e.clone().into()),
    }
}

enum KeyMode {
    Slice,
    Unsigned,
    Signed,
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
    let bits = builder.size_bits();
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
