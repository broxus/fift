use std::io::{BufRead, Write};
use std::num::NonZeroU32;
use std::rc::Rc;

use crate::continuation::*;
use crate::dictionary::*;
use crate::error::*;
use crate::lexer::*;
use crate::stack::*;

pub struct Context<'a> {
    pub state: State,
    pub stack: Stack,
    pub exit_code: u8,
    pub next: Option<Continuation>,
    pub dictionary: Dictionary,

    pub input: Lexer<'a>,
    pub stdout: &'a mut dyn Write,
}

impl<'a> Context<'a> {
    pub fn new(input: &'a mut dyn BufRead, stdout: &'a mut dyn Write) -> Self {
        Self {
            state: Default::default(),
            stack: Stack::new(None),
            exit_code: 0,
            next: None,
            dictionary: Default::default(),
            input: Lexer::new(input),
            stdout,
        }
    }

    pub fn run(&mut self) -> FiftResult<u8> {
        let mut current = Some(Rc::new(InterpretCont) as Continuation);
        while let Some(cont) = current.take() {
            current = cont.run(self)?;
            if current.is_none() {
                current = self.next.take();
            }
        }

        Ok(self.exit_code)
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

    pub fn begin_compile(&mut self) -> FiftResult<()> {
        match self {
            Self::Interpret => {
                *self = Self::Compile(NonZeroU32::MIN);
                Ok(())
            }
            Self::Compile(depth) => {
                *depth = depth.checked_add(1).ok_or(FiftError::IntegerOverflow)?;
                Ok(())
            }
            Self::InterpretInternal(_) => Err(FiftError::ExpectedNonInternalInterpreterMode),
        }
    }

    pub fn end_compile(&mut self) -> FiftResult<()> {
        if let Self::Compile(depth) = self {
            match NonZeroU32::new(depth.get() - 1) {
                Some(new_depth) => *depth = new_depth,
                None => *self = Self::Interpret,
            }
            Ok(())
        } else {
            Err(FiftError::ExpectedCompilationMode)
        }
    }

    pub fn begin_interpret_internal(&mut self) -> FiftResult<()> {
        if let Self::Compile(depth) = self {
            *self = Self::InterpretInternal(*depth);
            Ok(())
        } else {
            Err(FiftError::ExpectedCompilationMode)
        }
    }

    pub fn end_interpret_internal(&mut self) -> FiftResult<()> {
        if let Self::InterpretInternal(depth) = self {
            *self = Self::Compile(*depth);
            Ok(())
        } else {
            Err(FiftError::ExpectedInternalInterpreterMode)
        }
    }
}
