use std::io::Write;
use std::num::NonZeroU32;
use std::rc::Rc;

use anyhow::{Context as _, Result};

pub use fift_proc::fift_module;

pub use self::cont::{Cont, ContImpl};
pub use self::dictionary::{Dictionaries, Dictionary, DictionaryEntry};
pub use self::env::{Environment, SourceBlock};
pub use self::lexer::{Lexer, Token};
pub use self::stack::{
    OwnedCellSlice, SharedBox, Stack, StackTuple, StackValue, StackValueType, WordList,
};

pub mod cont;
pub mod dictionary;
pub mod env;
pub mod lexer;
pub mod stack;

pub struct Context<'a> {
    pub state: State,
    pub stack: Stack,
    pub exit_code: u8,
    pub next: Option<Cont>,
    pub dicts: Dictionaries,

    pub input: Lexer,
    pub exit_interpret: SharedBox,

    pub env: &'a mut dyn Environment,
    pub stdout: &'a mut dyn Write,
}

impl<'a> Context<'a> {
    pub fn new(env: &'a mut dyn Environment, stdout: &'a mut dyn Write) -> Self {
        Self {
            state: Default::default(),
            stack: Stack::new(None),
            exit_code: 0,
            next: None,
            dicts: Default::default(),
            input: Default::default(),
            exit_interpret: Default::default(),
            env,
            stdout,
        }
    }

    pub fn with_module<T: Module>(mut self, module: T) -> Result<Self> {
        self.add_module(module)?;
        Ok(self)
    }

    pub fn add_module<T: Module>(&mut self, module: T) -> Result<()> {
        module.init(&mut self.dicts.current)
    }

    pub fn with_source_block(mut self, block: SourceBlock) -> Self {
        self.add_source_block(block);
        self
    }

    pub fn add_source_block(&mut self, block: SourceBlock) {
        self.input.push_source_block(block);
    }

    pub fn run(&mut self) -> Result<u8> {
        let mut current = Some(Rc::new(cont::InterpreterCont) as Cont);
        while let Some(cont) = current.take() {
            current = cont.run(self)?;
            if current.is_none() {
                current = self.next.take();
            }
        }

        Ok(self.exit_code)
    }

    pub(crate) fn execute_stack_top(&mut self) -> Result<Cont> {
        let cont = self.stack.pop_cont()?;
        let count = self.stack.pop_smallint_range(0, 255)? as usize;
        self.stack.check_underflow(count)?;
        Ok(cont.as_ref().clone())
    }

    pub(crate) fn compile_stack_top(&mut self) -> Result<()> {
        let word_def = self.stack.pop_cont()?;
        let count = self.stack.pop_smallint_range(0, 255)? as usize;

        let cont = match count {
            0 => None,
            1 => Some(Rc::new(cont::LitCont(self.stack.pop()?)) as Cont),
            _ => {
                let mut literals = Vec::with_capacity(count);
                for _ in 0..count {
                    literals.push(self.stack.pop()?);
                }
                literals.reverse();
                Some(Rc::new(cont::MultiLitCont(literals)) as Cont)
            }
        };

        let mut word_list = self.stack.pop_word_list()?;
        {
            let word_list = Rc::make_mut(&mut word_list);
            word_list.items.extend(cont);

            if !cont::NopCont::is_nop(&**word_def) {
                word_list.items.push(Rc::clone(&word_def));
            }
        }

        self.stack.push_raw(word_list)
    }
}

#[derive(Debug, Default)]
pub enum State {
    #[default]
    Interpret,
    Compile(NonZeroU32),
    InterpretInternal(NonZeroU32),
}

impl State {
    pub fn is_compile(&self) -> bool {
        matches!(self, Self::Compile(_))
    }

    pub fn begin_compile(&mut self) -> Result<()> {
        match self {
            Self::Interpret => {
                *self = Self::Compile(NonZeroU32::MIN);
                Ok(())
            }
            Self::Compile(depth) => {
                *depth = depth.checked_add(1).context("Compiler depth overflow")?;
                Ok(())
            }
            Self::InterpretInternal(_) => anyhow::bail!("Expected non-internal interpreter mode"),
        }
    }

    pub fn end_compile(&mut self) -> Result<()> {
        if let Self::Compile(depth) = self {
            match NonZeroU32::new(depth.get() - 1) {
                Some(new_depth) => *depth = new_depth,
                None => *self = Self::Interpret,
            }
            Ok(())
        } else {
            anyhow::bail!("Expected compilation mode")
        }
    }

    pub fn begin_interpret_internal(&mut self) -> Result<()> {
        if let Self::Compile(depth) = self {
            *self = Self::InterpretInternal(*depth);
            Ok(())
        } else {
            anyhow::bail!("Expected compilation mode")
        }
    }

    pub fn end_interpret_internal(&mut self) -> Result<()> {
        if let Self::InterpretInternal(depth) = self {
            *self = Self::Compile(*depth);
            Ok(())
        } else {
            anyhow::bail!("Expected internal interpreter mode")
        }
    }
}

pub trait Module {
    fn init(&self, d: &mut Dictionary) -> Result<()>;
}

impl<T: Module> Module for &T {
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        T::init(self, d)
    }
}
