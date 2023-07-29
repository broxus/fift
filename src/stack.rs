use dyn_clone::DynClone;
use everscale_types::cell::OwnedCellSlice;
use everscale_types::prelude::*;
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

use crate::continuation::*;
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

    pub fn check_underflow(&self, n: usize) -> FiftResult<()> {
        if n > self.items.len() {
            Err(FiftError::StackUnderflow)
        } else {
            Ok(())
        }
    }

    pub fn fetch(&self, idx: usize) -> FiftResult<Box<dyn StackValue>> {
        let len = self.items.len();
        if idx < len {
            let item = self.items[len - idx - 1].as_ref();
            Ok(dyn_clone::clone_box(item))
        } else {
            Err(FiftError::StackUnderflow)
        }
    }

    pub fn swap(&mut self, lhs: usize, rhs: usize) -> FiftResult<()> {
        let len = self.items.len();
        if lhs >= len || rhs >= len {
            return Err(FiftError::StackUnderflow);
        }
        self.items.swap(len - lhs - 1, len - rhs - 1);
        Ok(())
    }

    pub fn push(&mut self, item: Box<dyn StackValue>) -> FiftResult<()> {
        if let Some(capacity) = &mut self.capacity {
            if self.items.len() >= *capacity {
                return Err(FiftError::StackOverflow);
            }
            *capacity += 1;
        }
        self.items.push(item);
        Ok(())
    }

    pub fn push_smallint(&mut self, value: u32) -> FiftResult<()> {
        self.push(Box::new(BigInt::from(value)))
    }

    pub fn push_argcount(&mut self, args: u32, cont: Continuation) -> FiftResult<()> {
        self.push(Box::new(BigInt::from(args)))?;
        self.push(Box::new(cont))
    }

    pub fn pop(&mut self) -> FiftResult<Box<dyn StackValue>> {
        self.items.pop().ok_or(FiftError::StackUnderflow)
    }

    pub fn pop_bool(&mut self) -> FiftResult<bool> {
        // TODO: use custom strange bool and different cast to bool
        let item = self.pop()?.into_int()?;
        Ok(!item.is_zero())
    }

    pub fn pop_smallint_range(&mut self, min: u32, max: u32) -> FiftResult<u32> {
        let item = self.pop()?.into_int()?;
        if let Some(item) = item.as_ref().to_u32() {
            if item <= max && item >= min {
                return Ok(item);
            }
        }
        Err(FiftError::ExpectedIntegerInRange)
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

pub struct StackValueDump<'a>(pub &'a dyn StackValue);

impl std::fmt::Display for StackValueDump<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.dump(f)
    }
}

pub struct StackValuePrintList<'a>(pub &'a dyn StackValue);

impl std::fmt::Display for StackValuePrintList<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.print_list(f)
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

            $(fn $cast(&self) -> FiftResult<&$cast_res> {
                Err(FiftError::InvalidType)
            })*

            $(fn $into(self: Box<Self>) -> FiftResult<Box<$ty>> {
                Err(FiftError::InvalidType)
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

            fn $cast(&self) -> FiftResult<&$cast_res> {
                let $cast_self = self;
                $cast_body
            }

            fn $into(self: Box<Self>) -> FiftResult<Box<$ty>> {
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
            as_slice(v): CellSlice = Ok(v.as_ref()),
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
                f.write_str("[ ")?;
                for item in v {
                    StackValue::dump(item.as_ref(), f)?;
                }
                f.write_str("]")
            },
            as_tuple(v): StackTuple = Ok(v),
            into_tuple,
        },
        Cont(Continuation) = {
            dump(_v, f) = {
                // TODO: dump content?
                f.write_str("Cont")
            },
            as_cont(v): Continuation = Ok(v),
            into_cont,
        }
    }
}

impl<'a> dyn StackValue + 'a {
    pub fn display_dump(&self) -> StackValueDump {
        StackValueDump(self)
    }

    pub fn display_list(&self) -> StackValuePrintList {
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
