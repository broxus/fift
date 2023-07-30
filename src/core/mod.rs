use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::num::NonZeroU32;
use std::rc::Rc;

pub use fift_proc::fift_module;

pub use self::cont::{Cont, ContImpl};
pub use self::dictionary::{Dictionary, DictionaryEntry};
pub use self::lexer::{Lexer, Token};
pub use self::stack::{Stack, StackTuple, StackValue, StackValueType, WordList};

use crate::error::*;
use crate::util::ImmediateInt;

pub mod cont;
pub mod dictionary;
pub mod lexer;
pub mod stack;

pub struct Context<'a> {
    pub state: State,
    pub stack: Stack,
    pub exit_code: u8,
    pub next: Option<Cont>,
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

    pub fn with_module<T: Module>(mut self, module: T) -> Result<Self> {
        self.add_module(module)?;
        Ok(self)
    }

    pub fn add_module<T: Module>(&mut self, module: T) -> Result<()> {
        module.init(&mut self.dictionary)
    }

    pub fn run(&mut self) -> Result<u8> {
        let mut current = Some(Rc::new(InterpreterCont) as Cont);
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
        Ok(*cont)
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
        word_list.items.extend(cont);

        if !self.dictionary.is_nop(&**word_def) {
            word_list.items.push(*word_def);
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
                *depth = depth.checked_add(1).ok_or(Error::IntegerOverflow)?;
                Ok(())
            }
            Self::InterpretInternal(_) => Err(Error::ExpectedNonInternalInterpreterMode),
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
            Err(Error::ExpectedCompilationMode)
        }
    }

    pub fn begin_interpret_internal(&mut self) -> Result<()> {
        if let Self::Compile(depth) = self {
            *self = Self::InterpretInternal(*depth);
            Ok(())
        } else {
            Err(Error::ExpectedCompilationMode)
        }
    }

    pub fn end_interpret_internal(&mut self) -> Result<()> {
        if let Self::InterpretInternal(depth) = self {
            *self = Self::Compile(*depth);
            Ok(())
        } else {
            Err(Error::ExpectedInternalInterpreterMode)
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

struct InterpreterCont;

impl ContImpl for InterpreterCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        use cont::SeqCont;

        thread_local! {
            static COMPILE_EXECUTE: Cont = Rc::new(CompileExecuteCont);
            static WORD: RefCell<String> = RefCell::new(String::with_capacity(128));
        };

        ctx.stdout.flush()?;

        let compile_exec = COMPILE_EXECUTE.with(|c| c.clone());

        'token: {
            let mut rewind = 0;
            let entry = 'entry: {
                let Some(token) = ctx.input.scan_word()? else {
                    return Ok(None);
                };

                // Find the largest subtoken first
                for subtoken in token.subtokens() {
                    if let Some(entry) = ctx.dictionary.lookup(subtoken) {
                        rewind = token.delta(subtoken);
                        break 'entry entry;
                    }
                }

                // Find in predefined entries
                if let Some(entry) = WORD.with(|word| {
                    let mut word = word.borrow_mut();
                    word.clear();
                    word.push_str(token.data);
                    word.push(' ');
                    ctx.dictionary.lookup(&word)
                }) {
                    break 'entry entry;
                }

                // Try parse as number
                if let Some(value) = ImmediateInt::try_from_str(token.data)? {
                    ctx.stack.push(value.num)?;
                    if let Some(denom) = value.denom {
                        ctx.stack.push(denom)?;
                        ctx.stack.push_argcount(2, ctx.dictionary.make_nop())?;
                    } else {
                        ctx.stack.push_argcount(1, ctx.dictionary.make_nop())?;
                    }
                    break 'token;
                }

                return Err(Error::UndefinedWord);
            };
            ctx.input.rewind(rewind);

            if entry.active {
                ctx.next = SeqCont::make(
                    Some(compile_exec),
                    SeqCont::make(Some(self), ctx.next.take()),
                );
                return Ok(Some(entry.definition.clone()));
            } else {
                ctx.stack.push_argcount(0, entry.definition.clone())?;
            }
        };

        // TODO: update `exec_interpret`

        ctx.next = SeqCont::make(Some(self), ctx.next.take());
        Ok(Some(compile_exec))
    }

    fn write_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<text interpreter continuation>")
    }
}

struct CompileExecuteCont;

impl ContImpl for CompileExecuteCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        Ok(if ctx.state.is_compile() {
            ctx.compile_stack_top()?;
            None
        } else {
            Some(ctx.execute_stack_top()?)
        })
    }

    fn write_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<compile execute continuation>")
    }
}
