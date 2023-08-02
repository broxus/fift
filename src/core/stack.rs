use std::cell::RefCell;
use std::rc::Rc;

use ahash::HashMap;
use anyhow::Result;
use dyn_clone::DynClone;
use everscale_types::cell::OwnedCellSlice;
use everscale_types::prelude::*;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use super::cont::*;

pub struct Stack {
    items: Vec<Box<dyn StackValue>>,
    capacity: Option<usize>,
    atoms: Atoms,
}

impl Stack {
    pub fn new(capacity: Option<usize>) -> Self {
        Self {
            items: Default::default(),
            capacity,
            atoms: Atoms::default(),
        }
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

    pub fn fetch(&self, idx: usize) -> Result<Box<dyn StackValue>> {
        let len = self.items.len();
        anyhow::ensure!(idx < len, StackError::StackUnderflow(idx));

        let item = self.items[len - idx - 1].as_ref();
        Ok(dyn_clone::clone_box(item))
    }

    pub fn swap(&mut self, lhs: usize, rhs: usize) -> Result<()> {
        let len = self.items.len();
        anyhow::ensure!(lhs < len, StackError::StackUnderflow(lhs));
        anyhow::ensure!(rhs < len, StackError::StackUnderflow(rhs));
        self.items.swap(len - lhs - 1, len - rhs - 1);
        Ok(())
    }

    pub fn push<T: StackValue + 'static>(&mut self, item: T) -> Result<()> {
        self.push_raw(Box::new(item))
    }

    pub fn push_raw(&mut self, item: Box<dyn StackValue>) -> Result<()> {
        if let Some(capacity) = &mut self.capacity {
            anyhow::ensure!(
                self.items.len() < *capacity,
                StackError::StackOverflow(*capacity)
            );
            *capacity += 1;
        }
        self.items.push(item);
        Ok(())
    }

    pub fn push_bool(&mut self, value: bool) -> Result<()> {
        self.push(if value {
            -BigInt::one()
        } else {
            BigInt::zero()
        })
    }

    pub fn push_int<T: Into<BigInt>>(&mut self, value: T) -> Result<()> {
        self.push(value.into())
    }

    pub fn push_argcount(&mut self, args: u32, cont: Cont) -> Result<()> {
        self.push_int(args)?;
        self.push(cont)
    }

    pub fn pop(&mut self) -> Result<Box<dyn StackValue>> {
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
        if let Some(item) = item.to_u32() {
            if item >= min && item <= max {
                return Ok(item);
            }
        }
        anyhow::bail!(StackError::IntegerOutOfRange {
            min,
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
        if let Some(item) = item.to_u32() {
            if item <= char::MAX as u32 {
                if let Some(char) = char::from_u32(item) {
                    return Ok(char);
                }
            }
        }
        anyhow::bail!(StackError::InvalidChar(item.to_string()))
    }

    pub fn pop_int(&mut self) -> Result<Box<BigInt>> {
        self.pop()?.into_int()
    }

    pub fn pop_string(&mut self) -> Result<Box<String>> {
        self.pop()?.into_string()
    }

    pub fn pop_bytes(&mut self) -> Result<Box<Vec<u8>>> {
        self.pop()?.into_bytes()
    }

    pub fn pop_cell(&mut self) -> Result<Box<Cell>> {
        self.pop()?.into_cell()
    }

    pub fn pop_builder(&mut self) -> Result<Box<CellBuilder>> {
        self.pop()?.into_builder()
    }

    pub fn pop_slice(&mut self) -> Result<Box<OwnedCellSlice>> {
        self.pop()?.into_slice()
    }

    pub fn pop_cont(&mut self) -> Result<Box<Cont>> {
        self.pop()?.into_cont()
    }

    pub fn pop_word_list(&mut self) -> Result<Box<WordList>> {
        self.pop()?.into_word_list()
    }

    pub fn pop_tuple(&mut self) -> Result<Box<StackTuple>> {
        self.pop()?.into_tuple()
    }

    pub fn pop_shared_box(&mut self) -> Result<Box<SharedBox>> {
        self.pop()?.into_shared_box()
    }

    pub fn pop_atom(&mut self) -> Result<Box<Atom>> {
        self.pop()?.into_atom()
    }

    pub fn items(&self) -> &[Box<dyn StackValue>] {
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
                    if first {
                        first = false;
                    } else {
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
                    if first {
                        first = false;
                    } else {
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
        }
    ),*$(,)?}) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
        pub enum $value_type {
            $($name),*,
        }

        pub trait $trait: DynClone {
            fn ty(&self) -> $value_type;

            fn is_equal(&self, other: &dyn $trait) -> bool;

            fn fmt_dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;

            $(fn $cast(&self) -> Result<&$cast_res> {
                Err(StackError::UnexpectedType {
                    expected: $value_type::$name,
                    actual: self.ty(),
                }.into())
            })*

            $(fn $into(self: Box<Self>) -> Result<Box<$ty>> {
                Err(StackError::UnexpectedType {
                    expected: $value_type::$name,
                    actual: self.ty(),
                }.into())
            })*
        }

        dyn_clone::clone_trait_object!($trait);

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

            fn $cast(&self) -> Result<&$cast_res> {
                let $cast_self = self;
                $cast_body
            }

            fn $into(self: Box<Self>) -> Result<Box<$ty>> {
                Ok(self)
            }
        })*
    };
}

define_stack_value! {
    StackValue(StackValueType), {
        Null(()) = {
            eq(_, _) = true,
            fmt_dump(_, f) = f.write_str("(null)"),
            as_null(v): () = Ok(v),
            into_null,
        },
        Int(BigInt) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = std::fmt::Display::fmt(v, f),
            as_int(v): BigInt = Ok(v),
            into_int,
        },
        Cell(Cell) = {
            eq(a, b) = a.as_ref() == b.as_ref(),
            fmt_dump(v, f) = write!(f, "C{{{}}}", v.repr_hash()),
            as_cell(v): Cell = Ok(v),
            into_cell,
        },
        Builder(CellBuilder) = {
            eq(a, b) = {
                // TODO: add `is_exotic` and check here
                a.bit_len() == b.bit_len()
                    && a.raw_data() == b.raw_data()
                    && a.references() == b.references()
            },
            fmt_dump(_v, f) = {
                // TODO: print builder data as hex
                f.write_str("BC{_data_}")
            },
            as_builder(v): CellBuilder = Ok(v),
            into_builder,
        },
        Slice(OwnedCellSlice) = {
            eq(a, b) = are_cell_slice_equal(a.pin(), b).unwrap_or_default(),
            fmt_dump(_v, f) = {
                // TODO: print slice data as hex
                f.write_str("CS{_data_}")
            },
            as_slice(v): CellSlice = Ok(v.pin()),
            into_slice,
        },
        String(String) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "\"{v}\""),
            as_string(v): String = Ok(v),
            into_string,
        },
        Bytes(Vec<u8>) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "BYTES:{}", hex::encode_upper(v)),
            as_bytes(v): Vec<u8> = Ok(v),
            into_bytes,
        },
        Tuple(StackTuple) = {
            eq(a, b) = {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.is_equal(b.as_ref()))
            },
            fmt_dump(v, f) = {
                if v.is_empty() {
                    return f.write_str("[]");
                }
                f.write_str("[")?;
                for item in v {
                    f.write_str(" ")?;
                    StackValue::fmt_dump(item.as_ref(), f)?;
                }
                f.write_str(" ]")
            },
            as_tuple(v): StackTuple = Ok(v),
            into_tuple,
        },
        Cont(Cont) = {
            eq(a, b) = {
                let a = Rc::as_ptr(a) as *const ();
                let b = Rc::as_ptr(b) as *const ();
                std::ptr::eq(a, b)
            },
            fmt_dump(v, f) = write!(f, "Cont{{{:?}}}", Rc::as_ptr(v)),
            as_cont(v): Cont = Ok(v),
            into_cont,
        },
        WordList(WordList) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = write!(f, "WordList{{{:?}}}", &v as *const _),
            as_word_list(v): WordList = Ok(v),
            into_word_list,
        },
        SharedBox(SharedBox) = {
            eq(a, b) = {
                let a = Rc::as_ptr(&a.value) as *const ();
                let b = Rc::as_ptr(&b.value) as *const ();
                std::ptr::eq(a, b)
            },
            fmt_dump(v, f) = write!(f, "Box{{{:?}}}", Rc::as_ptr(&v.value)),
            as_box(v): SharedBox = Ok(v),
            into_shared_box,
        },
        Atom(Atom) = {
            eq(a, b) = a == b,
            fmt_dump(v, f) = std::fmt::Display::fmt(v, f),
            as_atom(v): Atom = Ok(v),
            into_atom,
        }
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

    fn as_pair(&self) -> Option<(&dyn StackValue, &dyn StackValue)> {
        let tuple = self.as_tuple().ok()?;
        match tuple.as_slice() {
            [first, second] => Some((first.as_ref(), second.as_ref())),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<(&dyn StackValue, &dyn StackValue)> {
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

            f.write_str("[ ")?;
            for item in tuple {
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

pub type StackTuple = Vec<Box<dyn StackValue>>;

#[derive(Default, Clone)]
pub struct WordList {
    pub items: Vec<Cont>,
}

impl WordList {
    pub fn finish(self) -> Cont {
        if self.items.len() == 1 {
            return self.items.into_iter().next().unwrap();
        }

        Rc::new(ListCont {
            after: None,
            list: Rc::new(self),
            pos: 0,
        })
    }
}

impl Eq for WordList {}

impl PartialEq for WordList {
    fn eq(&self, other: &Self) -> bool {
        self.items.len() == other.items.len()
            && self.items.iter().zip(other.items.iter()).all(|(a, b)| {
                let a = Rc::as_ptr(a) as *const ();
                let b = Rc::as_ptr(b) as *const ();
                std::ptr::eq(a, b)
            })
    }
}

#[derive(Clone)]
pub struct SharedBox {
    value: Rc<RefCell<Box<dyn StackValue>>>,
}

impl Default for SharedBox {
    fn default() -> Self {
        Self::new(Box::new(()))
    }
}

impl SharedBox {
    pub fn new(value: Box<dyn StackValue>) -> Self {
        Self {
            value: Rc::new(RefCell::new(value)),
        }
    }

    pub fn store(&self, value: Box<dyn StackValue>) {
        *self.value.borrow_mut() = value;
    }

    pub fn fetch(&self) -> Box<dyn StackValue> {
        self.value.borrow().clone()
    }
}

#[derive(Clone, Eq, PartialEq)]
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

fn are_cell_slice_equal(a: &CellSlice, b: &CellSlice) -> Result<bool> {
    if a.remaining_bits() != b.remaining_bits() || a.remaining_refs() != b.remaining_refs() {
        return Ok(false);
    }
    if a.references().zip(b.references()).any(|(a, b)| a != b) {
        return Ok(false);
    }

    // TODO: add tests
    let bits = a.remaining_bits();
    let rem = bits % 32;
    for offset in (0..bits - rem).step_by(32) {
        if a.get_u32(offset)? != b.get_u32(offset)? {
            return Ok(false);
        }
    }

    Ok(rem == 0 || a.get_uint(bits - rem, rem)? == b.get_uint(bits - rem, rem)?)
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
    #[error("Expected a valid utf8 char code, found {0}")]
    InvalidChar(String),
}
