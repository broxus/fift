use std::rc::Rc;

use dyn_clone::DynClone;
use everscale_types::cell::OwnedCellSlice;
use everscale_types::prelude::*;
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::cont::*;
use crate::error::*;

pub struct Stack {
    items: Vec<Box<dyn StackValue>>,
    capacity: Option<usize>,
}

impl Stack {
    pub fn new(capacity: Option<usize>) -> Self {
        Self {
            items: Default::default(),
            capacity,
        }
    }

    pub fn depth(&self) -> usize {
        self.items.len()
    }

    pub fn check_underflow(&self, n: usize) -> Result<()> {
        if n > self.items.len() {
            Err(Error::StackUnderflow)
        } else {
            Ok(())
        }
    }

    pub fn fetch(&self, idx: usize) -> Result<Box<dyn StackValue>> {
        let len = self.items.len();
        if idx < len {
            let item = self.items[len - idx - 1].as_ref();
            Ok(dyn_clone::clone_box(item))
        } else {
            Err(Error::StackUnderflow)
        }
    }

    pub fn swap(&mut self, lhs: usize, rhs: usize) -> Result<()> {
        let len = self.items.len();
        if lhs >= len || rhs >= len {
            return Err(Error::StackUnderflow);
        }
        self.items.swap(len - lhs - 1, len - rhs - 1);
        Ok(())
    }

    pub fn push<T: StackValue + 'static>(&mut self, item: T) -> Result<()> {
        self.push_raw(Box::new(item))
    }

    pub fn push_raw(&mut self, item: Box<dyn StackValue>) -> Result<()> {
        if let Some(capacity) = &mut self.capacity {
            if self.items.len() >= *capacity {
                return Err(Error::StackOverflow);
            }
            *capacity += 1;
        }
        self.items.push(item);
        Ok(())
    }

    pub fn push_bool(&mut self, value: bool) -> Result<()> {
        self.push(BigInt::from(if value {
            -BigInt::one()
        } else {
            BigInt::zero()
        }))
    }

    pub fn push_int<T: Into<BigInt>>(&mut self, value: T) -> Result<()> {
        self.push(value.into())
    }

    pub fn push_argcount(&mut self, args: u32, cont: Cont) -> Result<()> {
        self.push_int(args)?;
        self.push(cont)
    }

    pub fn pop(&mut self) -> Result<Box<dyn StackValue>> {
        self.items.pop().ok_or(Error::StackUnderflow)
    }

    pub fn pop_bool(&mut self) -> Result<bool> {
        let item = self.pop_int()?;
        Ok(item.is_negative())
    }

    pub fn pop_smallint_range(&mut self, min: u32, max: u32) -> Result<u32> {
        let item = self.pop_int()?;
        if let Some(item) = item.to_u32() {
            if item <= max && item >= min {
                return Ok(item);
            }
        }
        Err(Error::ExpectedIntegerInRange)
    }

    pub fn pop_usize(&mut self) -> Result<usize> {
        self.pop_int()?
            .to_usize()
            .ok_or(Error::ExpectedIntegerInRange)
    }

    pub fn pop_smallint_char(&mut self) -> Result<char> {
        char::from_u32(self.pop_smallint_range(0, char::MAX as u32)?).ok_or(Error::InvalidChar)
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

    pub fn display_dump(&self) -> impl std::fmt::Display + '_ {
        struct StackDump<'a>(&'a Stack);

        impl std::fmt::Display for StackDump<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                for item in &self.0.items {
                    if first {
                        first = false;
                    } else {
                        f.write_str(" ")?;
                    }
                    item.as_ref().dump(f)?;
                }
                Ok(())
            }
        }

        StackDump(self)
    }

    pub fn display_list(&self) -> impl std::fmt::Display + '_ {
        struct StackPrintList<'a>(&'a Stack);

        impl std::fmt::Display for StackPrintList<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                for item in &self.0.items {
                    if first {
                        first = false;
                    } else {
                        f.write_str(" ")?;
                    }
                    item.as_ref().print_list(f)?;
                }
                Ok(())
            }
        }

        StackPrintList(self)
    }
}

macro_rules! define_stack_value {
    ($trait:ident($value_type:ident), {$(
        $name:ident($ty:ty) = {
            dump($dump_self:ident, $f:ident) = $dump_body:expr,
            $cast:ident($cast_self:ident): $cast_res:ty = $cast_body:expr,
            $into:ident$(,)?
        }
    ),*$(,)?}) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
        pub enum $value_type {
            $($name),*,
        }

        pub trait $trait: DynClone {
            fn ty(&self) -> $value_type;

            fn dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;

            $(fn $cast(&self) -> Result<&$cast_res> {
                Err(Error::InvalidType)
            })*

            $(fn $into(self: Box<Self>) -> Result<Box<$ty>> {
                Err(Error::InvalidType)
            })*
        }

        dyn_clone::clone_trait_object!($trait);

        $(impl $trait for $ty {
            fn ty(&self) -> $value_type {
                $value_type::$name
            }

            fn dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let $dump_self = self;
                let $f = f;
                $dump_body
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
            dump(_v, f) = {
                f.write_str("(null)")
            },
            as_null(v): () = Ok(v),
            into_null,
        },
        Int(BigInt) = {
            dump(v, f) = {
                std::fmt::Display::fmt(v, f)
            },
            as_int(v): BigInt = Ok(v),
            into_int,
        },
        Cell(Cell) = {
            dump(v, f) = {
                write!(f, "C{{{}}}", v.repr_hash())
            },
            as_cell(v): Cell = Ok(v),
            into_cell,
        },
        Builder(CellBuilder) = {
            dump(_v, f) = {
                // TODO: print builder data as hex
                f.write_str("BC{_data_}")
            },
            as_builder(v): CellBuilder = Ok(v),
            into_builder,
        },
        Slice(OwnedCellSlice) = {
            dump(_v, f) = {
                // TODO: print slice data as hex
                f.write_str("CS{_data_}")
            },
            as_slice(v): CellSlice = Ok(v.pin()),
            into_slice,
        },
        String(String) = {
            dump(v, f) = {
                write!(f, "\"{v}\"")
            },
            as_string(v): String = Ok(v),
            into_string,
        },
        Bytes(Vec<u8>) = {
            dump(v, f) = {
                write!(f, "BYTES:{}", hex::encode_upper(v))
            },
            as_bytes(v): Vec<u8> = Ok(v),
            into_bytes,
        },
        Tuple(StackTuple) = {
            dump(v, f) = {
                if v.is_empty() {
                    return f.write_str("[]");
                }
                f.write_str("[")?;
                for item in v {
                    f.write_str(" ")?;
                    StackValue::dump(item.as_ref(), f)?;
                }
                f.write_str(" ]")
            },
            as_tuple(v): StackTuple = Ok(v),
            into_tuple,
        },
        Cont(Cont) = {
            dump(_v, f) = {
                // TODO: dump content?
                f.write_str("Cont")
            },
            as_cont(v): Cont = Ok(v),
            into_cont,
        },
        WordList(WordList) = {
            dump(_v, f) = {
                // f.write_str("{")?;
                // for item in &self.items {
                //     write!(f, " {}", item.display_name(d))?;
                // }
                // f.write_str("}")
                f.write_str("WordList")
            },
            as_word_list(v): WordList = Ok(v),
            into_word_list,
        }
    }
}

impl dyn StackValue + '_ {
    pub fn display_dump(&self) -> impl std::fmt::Display + '_ {
        pub struct StackValueDump<'a>(pub &'a dyn StackValue);

        impl std::fmt::Display for StackValueDump<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.dump(f)
            }
        }

        StackValueDump(self)
    }

    pub fn display_list(&self) -> impl std::fmt::Display + '_ {
        pub struct StackValuePrintList<'a>(pub &'a dyn StackValue);

        impl std::fmt::Display for StackValuePrintList<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.print_list(f)
            }
        }

        StackValuePrintList(self)
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

    pub fn print_list(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_null() {
            f.write_str("()")
        } else if let Ok(tuple) = self.as_tuple() {
            if let Some((head, tail)) = self.as_list() {
                f.write_str("(")?;
                head.print_list(f)?;
                tail.print_list_tail(f)?;
                return Ok(());
            }

            f.write_str("[ ")?;
            for item in tuple {
                item.as_ref().print_list(f)?;
            }
            f.write_str("]")?;

            Ok(())
        } else {
            self.dump(f)
        }
    }

    fn print_list_tail(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut item = self;
        while !item.is_null() {
            let Some((head, tail)) = item.as_pair() else {
                f.write_str(" . ")?;
                item.print_list(f)?;
                break;
            };

            f.write_str(" ")?;
            head.print_list(f)?;
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
