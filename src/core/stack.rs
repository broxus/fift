use std::cell::RefCell;
use std::rc::Rc;

use ahash::HashMap;
use anyhow::Result;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};
use tycho_types::prelude::*;
pub use tycho_vm::OwnedCellSlice;
use tycho_vm::{SafeDelete, SafeRc, SafeRcMakeMut};

use super::cont::*;

pub struct Stack {
    items: Vec<SafeRc<dyn StackValue>>,
    capacity: Option<usize>,
    atoms: Atoms,
}

impl Stack {
    pub fn make_null() -> SafeRc<dyn StackValue> {
        thread_local! {
            static NULL: SafeRc<dyn StackValue> = SafeRc::new_dyn_fift_value(());
        }
        NULL.with(|v| v.clone())
    }

    pub fn make_nan() -> SafeRc<dyn StackValue> {
        thread_local! {
            static NAN: SafeRc<dyn StackValue> = SafeRc::new_dyn_fift_value(tycho_vm::NaN);
        }
        NAN.with(|v| v.clone())
    }

    pub fn new(capacity: Option<usize>) -> Self {
        Self {
            items: Default::default(),
            capacity,
            atoms: Atoms::default(),
        }
    }

    pub fn take_items(&mut self) -> Vec<SafeRc<dyn StackValue>> {
        std::mem::take(&mut self.items)
    }

    pub fn set_items(&mut self, items: Vec<SafeRc<dyn StackValue>>) {
        self.items = items;
    }

    pub fn depth(&self) -> usize {
        self.items.len()
    }

    pub fn atoms(&self) -> &Atoms {
        &self.atoms
    }

    pub fn atoms_mut(&mut self) -> &mut Atoms {
        &mut self.atoms
    }

    pub fn check_underflow(&self, n: usize) -> Result<()> {
        anyhow::ensure!(n <= self.items.len(), StackError::StackUnderflow(n - 1));
        Ok(())
    }

    pub fn fetch(&self, idx: usize) -> Result<SafeRc<dyn StackValue>> {
        let len = self.items.len();
        anyhow::ensure!(idx < len, StackError::StackUnderflow(idx));
        Ok(self.items[len - idx - 1].clone())
    }

    pub fn swap(&mut self, lhs: usize, rhs: usize) -> Result<()> {
        let len = self.items.len();
        anyhow::ensure!(lhs < len, StackError::StackUnderflow(lhs));
        anyhow::ensure!(rhs < len, StackError::StackUnderflow(rhs));
        self.items.swap(len - lhs - 1, len - rhs - 1);
        // eprintln!("AFTER SWAP: {}", self.display_dump());
        Ok(())
    }

    pub fn push<T: StackValue + 'static>(&mut self, item: T) -> Result<()> {
        self.push_raw(SafeRc::new_dyn_fift_value(item))
    }

    pub fn push_raw(&mut self, item: SafeRc<dyn StackValue>) -> Result<()> {
        if let Some(capacity) = &mut self.capacity {
            anyhow::ensure!(
                self.items.len() < *capacity,
                StackError::StackOverflow(*capacity)
            );
            *capacity += 1;
        }
        self.items.push(item);
        // eprintln!("AFTER PUSH: {}", self.display_dump());
        Ok(())
    }

    pub fn extend_raw<T>(&mut self, items: T) -> Result<()>
    where
        T: IntoIterator,
        T::Item: Into<SafeRc<dyn StackValue>>,
    {
        for item in items {
            self.push_raw(item.into())?;
        }

        Ok(())
    }

    pub fn push_null(&mut self) -> Result<()> {
        self.push_raw(Self::make_null())
    }

    pub fn push_bool(&mut self, value: bool) -> Result<()> {
        self.push(if value {
            -BigInt::one()
        } else {
            BigInt::zero()
        })
    }

    pub fn push_opt<T: StackValue + 'static>(&mut self, value: Option<T>) -> Result<()> {
        match value {
            None => self.push_null(),
            Some(value) => self.push(value),
        }
    }

    pub fn push_opt_raw<T: StackValue + 'static>(
        &mut self,
        value: Option<SafeRc<T>>,
    ) -> Result<()> {
        match value {
            None => self.push_null(),
            Some(value) => self.push_raw(value.into_dyn_fift_value()),
        }
    }

    pub fn push_int<T: Into<BigInt>>(&mut self, value: T) -> Result<()> {
        self.push(value.into())
    }

    pub fn push_argcount(&mut self, args: u32) -> Result<()> {
        self.push_int(args)?;
        self.push_raw(NopCont::value_instance())
    }

    pub fn pop(&mut self) -> Result<SafeRc<dyn StackValue>> {
        // eprintln!("BEFORE POP: {}", self.display_dump());
        self.items
            .pop()
            .ok_or(StackError::StackUnderflow(0))
            .map_err(From::from)
    }

    pub fn pop_bool(&mut self) -> Result<bool> {
        Ok(!self.pop_int()?.is_zero())
    }

    pub fn pop_smallint_range(&mut self, min: u32, max: u32) -> Result<u32> {
        let item = self.pop_int()?;
        if let Some(item) = item.to_u32()
            && item >= min
            && item <= max
        {
            return Ok(item);
        }
        anyhow::bail!(StackError::IntegerOutOfRange {
            min,
            max: max as usize,
            actual: item.to_string(),
        })
    }

    pub fn pop_smallint_signed_range(&mut self, min: i32, max: i32) -> Result<i32> {
        let item = self.pop_int()?;
        if let Some(item) = item.to_i32()
            && item >= min
            && item <= max
        {
            return Ok(item);
        }
        anyhow::bail!(StackError::IntegerOutOfSignedRange {
            min: min as isize,
            max: max as isize,
            actual: item.to_string(),
        })
    }

    pub fn pop_long_range(&mut self, min: u64, max: u64) -> Result<u64> {
        let item = self.pop_int()?;
        if let Some(item) = item.to_u64()
            && item >= min
            && item <= max
        {
            return Ok(item);
        }
        anyhow::bail!(StackError::IntegerOutOfRange {
            min: min as _,
            max: max as usize,
            actual: item.to_string(),
        })
    }

    pub fn pop_usize(&mut self) -> Result<usize> {
        let item = self.pop_int()?;
        if let Some(item) = item.to_usize() {
            return Ok(item);
        }
        anyhow::bail!(StackError::IntegerOutOfRange {
            min: 0,
            max: usize::MAX,
            actual: item.to_string(),
        })
    }

    pub fn pop_smallint_char(&mut self) -> Result<char> {
        let item = self.pop_int()?;
        if let Some(item) = item.to_u32()
            && item <= char::MAX as u32
            && let Some(char) = char::from_u32(item)
        {
            return Ok(char);
        }
        anyhow::bail!(StackError::InvalidChar(item.to_string()))
    }

    pub fn pop_int(&mut self) -> Result<SafeRc<BigInt>> {
        self.pop()?.into_int()
    }

    pub fn pop_string(&mut self) -> Result<SafeRc<String>> {
        self.pop()?.into_string()
    }

    pub fn pop_string_owned(&mut self) -> Result<String> {
        Ok(match SafeRc::try_unwrap(self.pop()?.into_string()?) {
            Ok(inner) => inner,
            Err(rc) => rc.as_ref().clone(),
        })
    }

    pub fn pop_bytes(&mut self) -> Result<SafeRc<Vec<u8>>> {
        self.pop()?.into_bytes()
    }

    pub fn pop_bytes_owned(&mut self) -> Result<Vec<u8>> {
        Ok(match SafeRc::try_unwrap(self.pop()?.into_bytes()?) {
            Ok(inner) => inner,
            Err(rc) => rc.as_ref().clone(),
        })
    }

    pub fn pop_cell(&mut self) -> Result<SafeRc<Cell>> {
        self.pop()?.into_cell()
    }

    pub fn pop_builder(&mut self) -> Result<SafeRc<CellBuilder>> {
        self.pop()?.into_builder()
    }

    pub fn pop_builder_owned(&mut self) -> Result<CellBuilder> {
        Ok(match SafeRc::try_unwrap(self.pop()?.into_builder()?) {
            Ok(inner) => inner,
            Err(rc) => rc.as_ref().clone(),
        })
    }

    pub fn pop_cell_slice(&mut self) -> Result<SafeRc<OwnedCellSlice>> {
        self.pop()?.into_cell_slice()
    }

    pub fn pop_cont(&mut self) -> Result<RcFiftCont> {
        self.pop()?.into_cont()
    }

    pub fn pop_word_list(&mut self) -> Result<SafeRc<WordList>> {
        self.pop()?.into_word_list()
    }

    pub fn pop_tuple(&mut self) -> Result<SafeRc<StackTuple>> {
        self.pop()?.into_tuple()
    }

    pub fn pop_tuple_owned(&mut self) -> Result<StackTuple> {
        Ok(match SafeRc::try_unwrap(self.pop()?.into_tuple()?) {
            Ok(inner) => inner,
            Err(rc) => rc.as_ref().clone(),
        })
    }

    pub fn pop_shared_box(&mut self) -> Result<SafeRc<SharedBox>> {
        self.pop()?.into_shared_box()
    }

    pub fn pop_atom(&mut self) -> Result<SafeRc<Atom>> {
        self.pop()?.into_atom()
    }

    pub fn pop_hashmap(&mut self) -> Result<Option<SafeRc<HashMapTreeNode>>> {
        let value = self.pop()?;
        if value.is_null() {
            Ok(None)
        } else {
            value.into_hashmap().map(Some)
        }
    }

    pub fn items(&self) -> &[SafeRc<dyn StackValue>] {
        &self.items
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn display_dump(&self) -> impl std::fmt::Display + '_ {
        struct DisplayDump<'a>(&'a Stack);

        impl std::fmt::Display for DisplayDump<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                for item in &self.0.items {
                    if !std::mem::take(&mut first) {
                        f.write_str(" ")?;
                    }
                    item.as_ref().fmt_dump(f)?;
                }
                Ok(())
            }
        }

        DisplayDump(self)
    }

    pub fn display_list(&self) -> impl std::fmt::Display + '_ {
        struct DisplayList<'a>(&'a Stack);

        impl std::fmt::Display for DisplayList<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                for item in &self.0.items {
                    if !std::mem::take(&mut first) {
                        f.write_str(" ")?;
                    }
                    item.as_ref().fmt_list(f)?;
                }
                Ok(())
            }
        }

        DisplayList(self)
    }
}

macro_rules! define_stack_value {
    ($trait:ident($value_type:ident), {$(
        $name:ident($ty:ty) = {
            eq($eq_self:pat, $eq_other:pat) = $eq_body:expr,
            fmt_dump($dump_self:pat, $f:pat) = $fmt_dump_body:expr,
            $cast:ident($cast_self:pat): $cast_res:ty = $cast_body:expr,
            $into:ident$(,)?
            $({ $($other:tt)* })?
        }
    ),*$(,)?}) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub enum $value_type {
            $($name),*,
        }

        pub trait $trait: SafeDelete {
            fn ty(&self) -> $value_type;

            fn is_equal(&self, other: &dyn $trait) -> bool;

            fn fmt_dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;

            $(fn $cast(&self) -> Result<$cast_res> {
                Err(StackError::UnexpectedType {
                    expected: $value_type::$name,
                    actual: self.ty(),
                }.into())
            })*

            $(fn $into(self: Rc<Self>) -> Result<Rc<$ty>> {
                Err(StackError::UnexpectedType {
                    expected: $value_type::$name,
                    actual: self.ty(),
                }.into())
            })*
        }

        $(impl $trait for $ty {
            fn ty(&self) -> $value_type {
                $value_type::$name
            }

            fn is_equal(&self, other: &dyn $trait) -> bool {
                match other.$cast() {
                    Ok($eq_other) => {
                        let $eq_self = self;
                        $eq_body
                    },
                    Err(_) => false,
                }
            }

            fn fmt_dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let $dump_self = self;
                let $f = f;
                $fmt_dump_body
            }

            fn $cast(&self) -> Result<$cast_res> {
                let $cast_self = self;
                $cast_body
            }

            fn $into(self: Rc<Self>) -> Result<Rc<$ty>> {
                Ok(self)
            }

            $($($other)*)?
        })*
    };
}

define_stack_value! {
    StackValue(StackValueType), {
        Null(()) = {
            eq(_, _) = true,
            fmt_dump(_, f) = f.write_str("(null)"),
            as_null(v): &() = Ok(v),
            rc_into_null,
        },
        NaN(tycho_vm::NaN) = {
            eq(_, _) = true,
            fmt_dump(_, f) = f.write_str("NaN"),
            as_nan(v): &tycho_vm::NaN = Ok(v),
            rc_into_nan,
        },
        Int(BigInt) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = std::fmt::Display::fmt(v, f),
            as_int(v): &BigInt = Ok(v),
            rc_into_int,
        },
        Cell(Cell) = {
            eq(a, b) = a.as_ref() == b.as_ref(),
            fmt_dump(v, f) = write!(f, "C{{{}}}", v.repr_hash()),
            as_cell(v): &Cell = Ok(v),
            rc_into_cell,
        },
        Builder(CellBuilder) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = {
                let bytes = v.size_bits().div_ceil(8);
                write!(f, "BC{{{}, bits={}}}", hex::encode(&v.raw_data()[..bytes as usize]), v.size_bits())
            },
            as_builder(v): &CellBuilder = Ok(v),
            rc_into_builder,
        },
        Slice(OwnedCellSlice) = {
            eq(a, b) = *a == b,
            fmt_dump(v, f) = std::fmt::Display::fmt(v, f),
            as_slice(v): CellSlice<'_> = Ok(v.apply()),
            rc_into_cell_slice,
        },
        String(String) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "\"{v}\""),
            as_string(v): &str = Ok(v),
            rc_into_string,
        },
        Bytes(Vec<u8>) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "BYTES:{}", hex::encode_upper(v)),
            as_bytes(v): &[u8] = Ok(v),
            rc_into_bytes,
        },
        Tuple(StackTuple) = {
            eq(a, b) = {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.is_equal(b.as_ref()))
            },
            fmt_dump(v, f) = {
                if v.is_empty() {
                    return f.write_str("[]");
                }
                f.write_str("[ ")?;
                let mut first = true;
                for item in v {
                    if !std::mem::take(&mut first) {
                        f.write_str(" ")?;
                    }
                    StackValue::fmt_dump(item.as_ref(), f)?;
                }
                f.write_str(" ]")
            },
            as_tuple(v): &StackTuple = Ok(v),
            rc_into_tuple,
        },
        Cont(dyn FiftCont) = {
            eq(a, b) = std::ptr::addr_eq(a, b),
            fmt_dump(v, f) = write!(f, "Cont{{{:?}}}", v as *const _ as *const ()),
            as_cont(v): &dyn FiftCont = Ok(v),
            rc_into_cont,
        },
        WordList(WordList) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "WordList{{{:?}}}", &v as *const _),
            as_word_list(v): &WordList = Ok(v),
            rc_into_word_list,
            {
                fn rc_into_cont(self: Rc<Self>) -> Result<Rc<dyn FiftCont>> {
                    Ok(SafeRc::into_inner(self.finish()))
                }
            }
        },
        SharedBox(SharedBox) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "Box{{{:?}}}", Rc::as_ptr(&v.value)),
            as_box(v): &SharedBox = Ok(v),
            rc_into_shared_box,
        },
        Atom(Atom) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = std::fmt::Display::fmt(v, f),
            as_atom(v): &Atom = Ok(v),
            rc_into_atom,
        },
        HashMap(HashMapTreeNode) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "HashMap{{{:?}}}", &v as *const _),
            as_hashmap(v): &HashMapTreeNode = Ok(v),
            rc_into_hashmap,
        },
        VmCont(tycho_vm::RcCont) = {
            eq(a, b) = SafeRc::ptr_eq(a, b),
            fmt_dump(v, f) = write!(f, "VmCont{{{:?}}}", v.as_ptr() as *const ()),
            as_vm_cont(v): &tycho_vm::RcCont = Ok(v),
            rc_into_vm_cont,
        },
    }
}

impl dyn StackValue + '_ {
    pub fn display_dump(&self) -> impl std::fmt::Display + '_ {
        pub struct DisplayDump<'a>(&'a dyn StackValue);

        impl std::fmt::Display for DisplayDump<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt_dump(f)
            }
        }

        DisplayDump(self)
    }

    pub fn display_list(&self) -> impl std::fmt::Display + '_ {
        pub struct DisplayList<'a>(&'a dyn StackValue);

        impl std::fmt::Display for DisplayList<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt_list(f)
            }
        }

        DisplayList(self)
    }

    pub fn is_null(&self) -> bool {
        self.ty() == StackValueType::Null
    }

    pub fn as_pair(&self) -> Option<(&dyn StackValue, &dyn StackValue)> {
        let tuple = self.as_tuple().ok()?;
        match tuple.as_slice() {
            [first, second] => Some((first.as_ref(), second.as_ref())),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<(&dyn StackValue, &dyn StackValue)> {
        let (head, tail) = self.as_pair()?;

        let mut next = tail;
        while !next.is_null() {
            let (_, tail) = next.as_pair()?;
            next = tail;
        }

        Some((head, tail))
    }

    fn fmt_list(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_null() {
            f.write_str("()")
        } else if let Ok(tuple) = self.as_tuple() {
            if let Some((head, tail)) = self.as_list() {
                f.write_str("(")?;
                head.fmt_list(f)?;
                tail.fmt_list_tail(f)?;
                return Ok(());
            }

            f.write_str("[")?;
            let mut first = true;
            for item in tuple {
                if !std::mem::take(&mut first) {
                    f.write_str(" ")?;
                }
                item.as_ref().fmt_list(f)?;
            }
            f.write_str("]")?;

            Ok(())
        } else {
            self.fmt_dump(f)
        }
    }

    fn fmt_list_tail(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut item = self;
        while !item.is_null() {
            let Some((head, tail)) = item.as_pair() else {
                f.write_str(" . ")?;
                item.fmt_list(f)?;
                break;
            };

            f.write_str(" ")?;
            head.fmt_list(f)?;
            item = tail;
        }
        f.write_str(")")
    }
}

pub trait DynFiftValue {
    fn new_dyn_fift_value<T: StackValue + 'static>(value: T) -> SafeRc<dyn StackValue>;

    fn into_int(self) -> Result<SafeRc<BigInt>>;
    fn into_string(self) -> Result<SafeRc<String>>;
    fn into_bytes(self) -> Result<SafeRc<Vec<u8>>>;
    fn into_cell(self) -> Result<SafeRc<Cell>>;
    fn into_builder(self) -> Result<SafeRc<CellBuilder>>;
    fn into_cell_slice(self) -> Result<SafeRc<OwnedCellSlice>>;
    fn into_cont(self) -> Result<RcFiftCont>;
    fn into_word_list(self) -> Result<SafeRc<WordList>>;
    fn into_tuple(self) -> Result<SafeRc<StackTuple>>;
    fn into_shared_box(self) -> Result<SafeRc<SharedBox>>;
    fn into_atom(self) -> Result<SafeRc<Atom>>;
    fn into_hashmap(self) -> Result<SafeRc<HashMapTreeNode>>;
    fn into_vm_cont(self) -> Result<tycho_vm::RcCont>;
}

impl DynFiftValue for SafeRc<dyn StackValue> {
    #[inline]
    fn new_dyn_fift_value<T: StackValue + 'static>(value: T) -> SafeRc<dyn StackValue> {
        let value: Rc<dyn StackValue> = Rc::new(value);
        SafeRc::from(value)
    }

    #[inline]
    fn into_int(self) -> Result<SafeRc<BigInt>> {
        Self::into_inner(self).rc_into_int().map(SafeRc::from)
    }

    fn into_string(self) -> Result<SafeRc<String>> {
        Self::into_inner(self).rc_into_string().map(SafeRc::from)
    }

    fn into_bytes(self) -> Result<SafeRc<Vec<u8>>> {
        Self::into_inner(self).rc_into_bytes().map(SafeRc::from)
    }

    fn into_cell(self) -> Result<SafeRc<Cell>> {
        Self::into_inner(self).rc_into_cell().map(SafeRc::from)
    }

    fn into_builder(self) -> Result<SafeRc<CellBuilder>> {
        Self::into_inner(self).rc_into_builder().map(SafeRc::from)
    }

    fn into_cell_slice(self) -> Result<SafeRc<OwnedCellSlice>> {
        Self::into_inner(self)
            .rc_into_cell_slice()
            .map(SafeRc::from)
    }

    fn into_cont(self) -> Result<RcFiftCont> {
        Self::into_inner(self).rc_into_cont().map(SafeRc::from)
    }

    fn into_word_list(self) -> Result<SafeRc<WordList>> {
        Self::into_inner(self).rc_into_word_list().map(SafeRc::from)
    }

    fn into_tuple(self) -> Result<SafeRc<StackTuple>> {
        Self::into_inner(self).rc_into_tuple().map(SafeRc::from)
    }

    fn into_shared_box(self) -> Result<SafeRc<SharedBox>> {
        Self::into_inner(self)
            .rc_into_shared_box()
            .map(SafeRc::from)
    }

    fn into_atom(self) -> Result<SafeRc<Atom>> {
        Self::into_inner(self).rc_into_atom().map(SafeRc::from)
    }

    fn into_hashmap(self) -> Result<SafeRc<HashMapTreeNode>> {
        Self::into_inner(self).rc_into_hashmap().map(SafeRc::from)
    }

    fn into_vm_cont(self) -> Result<tycho_vm::RcCont> {
        Self::into_inner(self)
            .rc_into_vm_cont()
            .map(Rc::unwrap_or_clone)
    }
}

pub trait IntoDynFiftValue {
    fn into_dyn_fift_value(self) -> SafeRc<dyn StackValue>;
}

impl<T: StackValue> IntoDynFiftValue for Rc<T> {
    #[inline]
    fn into_dyn_fift_value(self) -> SafeRc<dyn StackValue> {
        let this: Rc<dyn StackValue> = self;
        SafeRc::from(this)
    }
}

impl<T: StackValue> IntoDynFiftValue for SafeRc<T> {
    #[inline]
    fn into_dyn_fift_value(self) -> SafeRc<dyn StackValue> {
        Rc::<T>::into_dyn_fift_value(SafeRc::into_inner(self))
    }
}

impl IntoDynFiftValue for RcFiftCont {
    #[inline]
    fn into_dyn_fift_value(self) -> SafeRc<dyn StackValue> {
        SafeRc::from(SafeRc::into_inner(self).rc_into_dyn_fift_value())
    }
}

pub trait RcIntoDynFiftValue {
    fn rc_into_dyn_fift_value(self: Rc<Self>) -> Rc<dyn StackValue>;
}

impl<T: StackValue> RcIntoDynFiftValue for T {
    #[inline]
    fn rc_into_dyn_fift_value(self: Rc<Self>) -> Rc<dyn StackValue> {
        self
    }
}

pub type StackTuple = Vec<SafeRc<dyn StackValue>>;

#[derive(Default, Clone)]
pub struct WordList {
    pub items: Vec<RcFiftCont>,
}

impl SafeRcMakeMut for WordList {
    #[inline]
    fn rc_make_mut(rc: &mut Rc<Self>) -> &mut Self {
        Rc::make_mut(rc)
    }
}

impl WordList {
    pub fn finish(self: Rc<Self>) -> RcFiftCont {
        if self.items.len() == 1 {
            return self.items.first().unwrap().clone();
        }

        RcFiftCont::new_dyn_fift_cont(ListCont {
            after: None,
            list: self,
            pos: 0,
        })
    }
}

impl Eq for WordList {}

impl PartialEq for WordList {
    fn eq(&self, other: &Self) -> bool {
        self.items.len() == other.items.len()
            && self
                .items
                .iter()
                .zip(other.items.iter())
                .all(|(a, b)| SafeRc::ptr_eq(a, b))
    }
}

#[derive(Clone)]
pub struct SharedBox {
    value: Rc<RefCell<SafeRc<dyn StackValue>>>,
}

impl Default for SharedBox {
    fn default() -> Self {
        Self::new(Stack::make_null())
    }
}

impl Eq for SharedBox {}
impl PartialEq for SharedBox {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
    }
}

impl SharedBox {
    pub fn new(value: SafeRc<dyn StackValue>) -> Self {
        Self {
            value: Rc::new(RefCell::new(value)),
        }
    }

    pub fn store(&self, value: SafeRc<dyn StackValue>) {
        *self.value.borrow_mut() = value;
    }

    pub fn store_opt<T: StackValue + 'static>(&self, value: Option<SafeRc<T>>) {
        *self.value.borrow_mut() = match value {
            None => Stack::make_null(),
            Some(value) => value.into_dyn_fift_value(),
        };
    }

    pub fn fetch(&self) -> SafeRc<dyn StackValue> {
        self.value.borrow().clone()
    }

    pub fn take(&self) -> SafeRc<dyn StackValue> {
        std::mem::replace(&mut *self.value.borrow_mut(), Stack::make_null())
    }

    pub fn borrow(&self) -> std::cell::Ref<'_, SafeRc<dyn StackValue>> {
        self.value.borrow()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Atom {
    Unnamed(i32),
    Named(Rc<str>),
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unnamed(idx) => write!(f, "atom#{idx}"),
            Self::Named(name) => name.fmt(f),
        }
    }
}

impl<T: AsRef<str>> PartialEq<T> for Atom {
    fn eq(&self, other: &T) -> bool {
        match self {
            Self::Unnamed(_) => false,
            Self::Named(name) => name.as_ref() == other.as_ref(),
        }
    }
}

#[derive(Default)]
pub struct Atoms {
    named: HashMap<Rc<str>, Atom>,
    total_anon: u32,
}

impl Atoms {
    pub fn clear(&mut self) {
        self.named.clear();
        self.total_anon = 0;
    }

    pub fn create_anon(&mut self) -> Atom {
        self.total_anon += 1;
        Atom::Unnamed(-(self.total_anon as i32))
    }

    pub fn create_named<T: AsRef<str>>(&mut self, name: T) -> Atom {
        if let Some(atom) = self.named.get(name.as_ref()) {
            return atom.clone();
        }

        let name = Rc::<str>::from(name.as_ref());
        let atom = Atom::Named(name.clone());
        self.named.insert(name, atom.clone());
        atom
    }

    pub fn get<T: AsRef<str>>(&self, name: T) -> Option<Atom> {
        self.named.get(name.as_ref()).cloned()
    }
}

#[derive(Clone)]
pub struct HashMapTreeNode {
    pub key: HashMapTreeKey,
    pub value: SafeRc<dyn StackValue>,
    pub left: Option<SafeRc<HashMapTreeNode>>,
    pub right: Option<SafeRc<HashMapTreeNode>>,
    pub rand_offset: u64,
}

impl SafeRcMakeMut for HashMapTreeNode {
    #[inline]
    fn rc_make_mut(rc: &mut Rc<Self>) -> &mut Self {
        Rc::make_mut(rc)
    }
}

impl Eq for HashMapTreeNode {}
impl PartialEq for HashMapTreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.value.is_equal(other.value.as_ref())
    }
}

impl HashMapTreeNode {
    pub fn new(key: HashMapTreeKey, value: SafeRc<dyn StackValue>) -> Self {
        Self {
            key,
            value,
            left: None,
            right: None,
            rand_offset: rand::random(),
        }
    }

    pub fn iter(&self) -> HashMapTreeIter<'_> {
        self.into_iter()
    }

    pub fn owned_iter(this: SafeRc<Self>) -> HashMapTreeOwnedIter {
        HashMapTreeOwnedIter {
            stack: vec![(this, None)],
        }
    }

    pub fn lookup<K>(root_opt: &Option<SafeRc<Self>>, key: K) -> Option<&'_ SafeRc<HashMapTreeNode>>
    where
        K: AsHashMapTreeKeyRef,
    {
        Self::lookup_internal(root_opt, &key.as_equivalent())
    }

    pub fn set(
        root_opt: &mut Option<SafeRc<Self>>,
        key: &HashMapTreeKey,
        value: &SafeRc<dyn StackValue>,
    ) {
        // TODO: insert new during replace
        if !key.stack_value.is_null()
            && !Self::replace(root_opt, key.as_equivalent(), value)
            && !value.is_null()
        {
            Self::insert_internal(root_opt, key, value, rand::random())
        }
    }

    pub fn replace<K>(
        root_opt: &mut Option<SafeRc<Self>>,
        key: K,
        value: &SafeRc<dyn StackValue>,
    ) -> bool
    where
        K: AsHashMapTreeKeyRef,
    {
        let key = key.as_equivalent();
        if key.stack_value.is_null() {
            false
        } else if value.is_null() {
            Self::remove_internal(root_opt, &key).is_some()
        } else if let Some(root) = root_opt {
            Self::replace_internal(root, &key, value)
        } else {
            false
        }
    }

    pub fn remove<K>(root_opt: &mut Option<SafeRc<Self>>, key: K) -> Option<SafeRc<dyn StackValue>>
    where
        K: AsHashMapTreeKeyRef,
    {
        let key = key.as_equivalent();
        if key.stack_value.is_null() {
            None
        } else {
            match Self::remove_internal(root_opt, &key) {
                Some(value) if !value.is_null() => Some(value),
                _ => None,
            }
        }
    }

    pub fn lookup_internal<'a>(
        root_opt: &'a Option<SafeRc<Self>>,
        key: &HashMapTreeKeyRef<'_>,
    ) -> Option<&'a SafeRc<HashMapTreeNode>> {
        let mut root = root_opt.as_ref()?;
        loop {
            root = match key.cmp_owned(&root.key) {
                std::cmp::Ordering::Equal => return Some(root),
                std::cmp::Ordering::Less => root.left.as_ref()?,
                std::cmp::Ordering::Greater => root.right.as_ref()?,
            };
        }
    }

    fn insert_internal(
        root_opt: &mut Option<SafeRc<Self>>,
        key: &HashMapTreeKey,
        value: &SafeRc<dyn StackValue>,
        rand_offset: u64,
    ) {
        let Some(mut root) = root_opt.take() else {
            *root_opt = Some(SafeRc::new(Self {
                key: key.clone(),
                value: value.clone(),
                left: None,
                right: None,
                rand_offset,
            }));
            return;
        };

        *root_opt = Some(if root.rand_offset <= rand_offset {
            let (left, right) = Self::split_internal(root, key);
            SafeRc::new(Self {
                key: key.clone(),
                value: value.clone(),
                left,
                right,
                rand_offset,
            })
        } else {
            let this = SafeRc::make_mut(&mut root);
            let branch = if key < &this.key {
                &mut this.left
            } else {
                &mut this.right
            };
            Self::insert_internal(branch, key, value, rand_offset);
            root
        });
    }

    fn replace_internal(
        this: &mut SafeRc<Self>,
        key: &HashMapTreeKeyRef<'_>,
        value: &SafeRc<dyn StackValue>,
    ) -> bool {
        fn replace_internal_impl(
            root: &mut SafeRc<HashMapTreeNode>,
            key: &HashMapTreeKeyRef<'_>,
            value: &SafeRc<dyn StackValue>,
        ) -> Option<()> {
            match key.cmp_owned(&root.key) {
                std::cmp::Ordering::Equal => {
                    let this = SafeRc::make_mut(root);
                    this.value = value.clone();
                }
                std::cmp::Ordering::Less => match SafeRc::get_mut(root) {
                    Some(this) => replace_internal_impl(this.left.as_mut()?, key, value)?,
                    None => {
                        let mut left = root.left.clone()?;
                        replace_internal_impl(&mut left, key, value)?;
                        SafeRc::make_mut(root).left = Some(left);
                    }
                },
                std::cmp::Ordering::Greater => match SafeRc::get_mut(root) {
                    Some(this) => replace_internal_impl(this.right.as_mut()?, key, value)?,
                    None => {
                        let mut right = root.right.clone()?;
                        replace_internal_impl(&mut right, key, value)?;
                        SafeRc::make_mut(root).right = Some(right);
                    }
                },
            }
            Some(())
        }

        replace_internal_impl(this, key, value).is_some()
    }

    fn remove_internal(
        root_opt: &mut Option<SafeRc<Self>>,
        key: &HashMapTreeKeyRef<'_>,
    ) -> Option<SafeRc<dyn StackValue>> {
        let new_root = {
            let root = root_opt.as_mut()?;
            match key.cmp_owned(&root.key) {
                std::cmp::Ordering::Equal => {
                    let (left, right) = match SafeRc::get_mut(root) {
                        Some(this) => (this.left.take(), this.right.take()),
                        None => (root.left.clone(), root.right.clone()),
                    };
                    Self::merge_internal(left, right)
                }
                std::cmp::Ordering::Less => {
                    return Some(match SafeRc::get_mut(root) {
                        Some(this) => Self::remove_internal(&mut this.left, key)?,
                        None => {
                            let mut left = root.left.clone();
                            let value = Self::remove_internal(&mut left, key)?;
                            SafeRc::make_mut(root).left = left;
                            value
                        }
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Some(match SafeRc::get_mut(root) {
                        Some(this) => Self::remove_internal(&mut this.right, key)?,
                        None => {
                            let mut right = root.right.clone();
                            let value = Self::remove_internal(&mut right, key)?;
                            SafeRc::make_mut(root).right = right;
                            value
                        }
                    });
                }
            }
        };

        let value = match SafeRc::try_unwrap(root_opt.take().unwrap()) {
            Ok(this) => this.value,
            Err(this) => this.value.clone(),
        };
        *root_opt = new_root;
        Some(value)
    }

    fn merge_internal(
        left: Option<SafeRc<Self>>,
        right: Option<SafeRc<Self>>,
    ) -> Option<SafeRc<Self>> {
        match (left, right) {
            (None, right) => right,
            (left, None) => left,
            (Some(mut left), Some(mut right)) => {
                if left.rand_offset > right.rand_offset {
                    let left_ref = SafeRc::make_mut(&mut left);
                    left_ref.right = Self::merge_internal(left_ref.right.take(), Some(right));
                    Some(left)
                } else {
                    let right_ref = SafeRc::make_mut(&mut right);
                    right_ref.left = Self::merge_internal(Some(left), right_ref.left.take());
                    Some(right)
                }
            }
        }
    }

    fn split_internal(
        mut this: SafeRc<Self>,
        key: &HashMapTreeKey,
    ) -> (Option<SafeRc<Self>>, Option<SafeRc<Self>>) {
        match key.cmp(&this.key) {
            std::cmp::Ordering::Less => {
                let Some(left) = (match SafeRc::get_mut(&mut this) {
                    Some(this) => this.left.take(),
                    None => this.left.clone(),
                }) else {
                    return (None, Some(this));
                };

                let (left, right) = Self::split_internal(left, key);
                SafeRc::make_mut(&mut this).left = right;
                (left, Some(this))
            }
            _ => {
                let Some(right) = (match SafeRc::get_mut(&mut this) {
                    Some(this) => this.right.take(),
                    None => this.right.clone(),
                }) else {
                    return (Some(this), None);
                };

                let (left, right) = Self::split_internal(right, key);
                SafeRc::make_mut(&mut this).right = left;
                (Some(this), right)
            }
        }
    }
}

impl<'a> IntoIterator for &'a HashMapTreeNode {
    type IntoIter = HashMapTreeIter<'a>;
    type Item = &'a HashMapTreeNode;

    fn into_iter(self) -> Self::IntoIter {
        HashMapTreeIter {
            stack: vec![(self, None)],
        }
    }
}

pub struct HashMapTreeIter<'a> {
    stack: Vec<(&'a HashMapTreeNode, Option<bool>)>,
}

impl<'a> Iterator for HashMapTreeIter<'a> {
    type Item = &'a HashMapTreeNode;

    fn next(&mut self) -> Option<Self::Item> {
        Some(loop {
            match self.stack.last_mut()? {
                (node, pos @ None) => {
                    *pos = Some(false);
                    break *node;
                }
                (node, pos @ Some(false)) => {
                    *pos = Some(true);
                    if let Some(next) = node.left.as_deref() {
                        self.stack.push((next, None))
                    }
                }
                (node, pos @ Some(true)) => {
                    if let Some(next) = node.right.as_deref() {
                        *node = next;
                        *pos = None;
                    } else {
                        self.stack.pop();
                    }
                }
            };
        })
    }
}

#[derive(Default, Clone)]
pub struct HashMapTreeOwnedIter {
    stack: Vec<(SafeRc<HashMapTreeNode>, Option<bool>)>,
}

impl Iterator for HashMapTreeOwnedIter {
    type Item = SafeRc<HashMapTreeNode>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(loop {
            match self.stack.last_mut()? {
                (node, pos @ None) => {
                    *pos = Some(false);
                    break node.clone();
                }
                (node, pos @ Some(false)) => {
                    *pos = Some(true);
                    if let Some(next) = node.left.clone() {
                        self.stack.push((next, None))
                    }
                }
                (node, pos @ Some(true)) => {
                    if let Some(next) = node.right.clone() {
                        *node = next;
                        *pos = None;
                    } else {
                        self.stack.pop();
                    }
                }
            };
        })
    }
}

pub trait AsHashMapTreeKeyRef {
    fn as_equivalent(&self) -> HashMapTreeKeyRef<'_>;
}

impl<T: AsHashMapTreeKeyRef> AsHashMapTreeKeyRef for &T {
    #[inline]
    fn as_equivalent(&self) -> HashMapTreeKeyRef<'_> {
        <T as AsHashMapTreeKeyRef>::as_equivalent(self)
    }
}

#[derive(Clone)]
pub struct HashMapTreeKey {
    pub hash: u64,
    pub stack_value: SafeRc<dyn StackValue>,
}

impl HashMapTreeKey {
    thread_local! {
        static HASHER_STATE: ahash::RandomState = ahash::RandomState::new();
    }

    pub fn new(value: SafeRc<dyn StackValue>) -> Result<Self> {
        let hash = Self::HASHER_STATE.with(|hasher| {
            Ok(match value.ty() {
                StackValueType::Null => 0,
                StackValueType::Int => hasher.hash_one(value.as_int()?),
                StackValueType::Atom => hasher.hash_one(value.as_atom()?),
                StackValueType::String => hasher.hash_one(value.as_string()?),
                StackValueType::Bytes => hasher.hash_one(value.as_bytes()?),
                ty => anyhow::bail!("Unsupported key type: {ty:?}"),
            })
        })?;

        Ok(Self {
            hash,
            stack_value: value,
        })
    }
}

impl From<String> for HashMapTreeKey {
    fn from(value: String) -> Self {
        Self {
            hash: Self::HASHER_STATE.with(|hasher| hasher.hash_one(&value)),
            stack_value: SafeRc::new_dyn_fift_value(value),
        }
    }
}

impl AsHashMapTreeKeyRef for HashMapTreeKey {
    fn as_equivalent(&self) -> HashMapTreeKeyRef<'_> {
        HashMapTreeKeyRef {
            hash: self.hash,
            stack_value: self.stack_value.as_ref(),
        }
    }
}

impl Ord for HashMapTreeKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_equivalent().cmp_owned(other)
    }
}

impl PartialOrd for HashMapTreeKey {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for HashMapTreeKey {}
impl PartialEq for HashMapTreeKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

#[derive(Clone, Copy)]
pub struct HashMapTreeKeyRef<'a> {
    hash: u64,
    stack_value: &'a dyn StackValue,
}

impl AsHashMapTreeKeyRef for HashMapTreeKeyRef<'_> {
    fn as_equivalent(&self) -> HashMapTreeKeyRef<'_> {
        *self
    }
}

impl<'a> From<&'a String> for HashMapTreeKeyRef<'a> {
    fn from(stack_value: &'a String) -> Self {
        HashMapTreeKey::HASHER_STATE.with(|hasher| Self {
            hash: hasher.hash_one(stack_value),
            stack_value,
        })
    }
}

impl HashMapTreeKeyRef<'_> {
    fn cmp_owned(&self, other: &HashMapTreeKey) -> std::cmp::Ordering {
        match self.hash.cmp(&other.hash) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        let ty = self.stack_value.ty();
        match ty.cmp(&other.stack_value.ty()) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        macro_rules! match_ty_cmp {
            ($ty: ident, { $($ident:ident => $cast:ident),*$(,)? }) => {
                match $ty {
                    $(StackValueType::$ident => {
                        if let (Ok(a), Ok(b)) = (self.stack_value.$cast(), other.stack_value.$cast()) {
                            return a.cmp(b);
                        }
                    })*
                    _ => {}
                }
            };
        }

        match_ty_cmp!(ty, {
            Int => as_int,
            Atom => as_atom,
            String => as_string,
            Bytes => as_bytes,
        });

        std::cmp::Ordering::Equal
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StackError {
    #[error("Stack underflow at depth {0}")]
    StackUnderflow(usize),
    #[error("Stack overflow with limit {0}")]
    StackOverflow(usize),
    #[error("Expected type `{expected:?}`, found type `{actual:?}`")]
    UnexpectedType {
        expected: StackValueType,
        actual: StackValueType,
    },
    #[error("Expected integer in range {min}..={max}, found {actual}")]
    IntegerOutOfRange {
        min: u32,
        max: usize,
        actual: String,
    },
    #[error("Expected integer in range {min}..={max}, found {actual}")]
    IntegerOutOfSignedRange {
        min: isize,
        max: isize,
        actual: String,
    },
    #[error("Expected a valid utf8 char code, found {0}")]
    InvalidChar(String),
}
