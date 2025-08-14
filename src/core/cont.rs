use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use num_bigint::BigInt;
use tycho_vm::{SafeDelete, SafeRc};

use super::stack::RcIntoDynFiftValue;
use super::{Context, Dictionary, Stack, StackValue, StackValueType, WordList};
use crate::core::IntoDynFiftValue;
use crate::util::*;

pub trait DynFiftCont {
    fn new_dyn_fift_cont<T: FiftCont + 'static>(cont: T) -> RcFiftCont;
}

impl DynFiftCont for RcFiftCont {
    #[inline]
    fn new_dyn_fift_cont<T: FiftCont + 'static>(cont: T) -> RcFiftCont {
        let cont: Rc<dyn FiftCont> = Rc::new(cont);
        RcFiftCont::from(cont)
    }
}

pub trait IntoDynFiftCont {
    fn into_dyn_fift_cont(self) -> RcFiftCont;
}

impl<T: FiftCont> IntoDynFiftCont for Rc<T> {
    #[inline]
    fn into_dyn_fift_cont(self) -> RcFiftCont {
        let this: Rc<dyn FiftCont> = self;
        RcFiftCont::from(this)
    }
}

impl<T: FiftCont> IntoDynFiftCont for SafeRc<T> {
    #[inline]
    fn into_dyn_fift_cont(self) -> RcFiftCont {
        Rc::<T>::into_dyn_fift_cont(SafeRc::into_inner(self))
    }
}

pub type RcFiftCont = SafeRc<dyn FiftCont>;

pub trait FiftCont: SafeDelete + RcIntoDynFiftValue {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>>;

    fn up(&self) -> Option<&RcFiftCont> {
        None
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;

    fn fmt_dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_name(d, f)
    }
}

impl dyn FiftCont + '_ {
    pub fn display_backtrace<'a>(&'a self, d: &'a Dictionary) -> impl std::fmt::Display + 'a {
        struct ContinuationBacktrace<'a> {
            d: &'a Dictionary,
            cont: &'a dyn FiftCont,
        }

        impl std::fmt::Display for ContinuationBacktrace<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut cont = self.cont;
                let mut newline = "";
                for i in 1..=16 {
                    write!(f, "{newline}{i:>4}: {}", cont.display_dump(self.d))?;
                    newline = "\n";
                    match cont.up() {
                        Some(next) => cont = next.as_ref(),
                        None => return Ok(()),
                    }
                }
                write!(f, "{newline}... more levels ...")
            }
        }

        ContinuationBacktrace { d, cont: self }
    }

    pub fn display_name<'a>(&'a self, d: &'a Dictionary) -> impl std::fmt::Display + 'a {
        struct ContinuationWriteName<'a> {
            d: &'a Dictionary,
            cont: &'a dyn FiftCont,
        }

        impl std::fmt::Display for ContinuationWriteName<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.cont.fmt_name(self.d, f)
            }
        }

        ContinuationWriteName { d, cont: self }
    }

    pub fn display_dump<'a>(&'a self, d: &'a Dictionary) -> impl std::fmt::Display + 'a {
        struct ContinuationDump<'a> {
            d: &'a Dictionary,
            cont: &'a dyn FiftCont,
        }

        impl std::fmt::Display for ContinuationDump<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.cont.fmt_dump(self.d, f)
            }
        }

        ContinuationDump { d, cont: self }
    }
}

impl<T: FiftCont + 'static> StackValue for T {
    fn ty(&self) -> StackValueType {
        StackValueType::Cont
    }

    fn is_equal(&self, other: &dyn StackValue) -> bool {
        StackValue::is_equal(self as &dyn FiftCont, other)
    }

    fn fmt_dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        StackValue::fmt_dump(self as &dyn FiftCont, f)
    }

    fn as_cont(&self) -> ::anyhow::Result<&dyn FiftCont> {
        Ok(self)
    }

    fn rc_into_cont(self: Rc<Self>) -> Result<Rc<dyn FiftCont>> {
        Ok(self)
    }
}

#[derive(Clone, Copy)]
pub struct InterpreterCont;

impl FiftCont for InterpreterCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        thread_local! {
            static COMPILE_EXECUTE: RcFiftCont = SafeRc::new_dyn_fift_cont(CompileExecuteCont);
            static WORD: RefCell<String> = RefCell::new(String::with_capacity(128));
        };

        let this = self.into_dyn_fift_cont();

        ctx.stdout.flush()?;

        let compile_exec = COMPILE_EXECUTE.with(|c| c.clone());

        'source_block: loop {
            'token: {
                let mut rewind = None;
                let entry = 'entry: {
                    let Some(token) = ctx.input.scan_word()? else {
                        if ctx.input.pop_source_block() {
                            continue 'source_block;
                        }
                        return Ok(None);
                    };

                    // Find in predefined entries
                    if let Some(entry) = WORD.with(|word| {
                        let mut word = word.borrow_mut();
                        word.clear();
                        word.push_str(token);
                        word.push(' ');

                        // Search parsed token as a separate word first
                        if let Some(entry) = ctx.dicts.lookup(&word, false)? {
                            return Ok::<_, anyhow::Error>(Some(entry));
                        }

                        // Then find the largest possible prefix
                        while !word.is_empty() {
                            word.pop();
                            if let Some(entry) = ctx.dicts.lookup(&word, false)? {
                                rewind = Some(word.len());
                                return Ok(Some(entry));
                            }
                        }

                        Ok(None)
                    })? {
                        break 'entry entry;
                    }

                    // Try parse as number
                    if let Some(value) = ImmediateInt::try_from_str(token)? {
                        ctx.stack.push(value.num)?;
                        if let Some(denom) = value.denom {
                            ctx.stack.push(denom)?;
                            ctx.stack.push_argcount(2)?;
                        } else {
                            ctx.stack.push_argcount(1)?;
                        }
                        break 'token;
                    }

                    anyhow::bail!("Undefined word `{token}`");
                };

                if let Some(rewind) = rewind {
                    ctx.input.rewind(rewind);
                } else {
                    ctx.input.skip_line_whitespace();
                }

                if entry.active {
                    ctx.next = SeqCont::make(
                        Some(compile_exec),
                        SeqCont::make(Some(this), ctx.next.take()),
                    );
                    return Ok(Some(entry.definition.clone()));
                } else {
                    ctx.stack.push_int(0)?;
                    ctx.stack.push_raw(entry.definition.into_dyn_fift_value())?;
                }
            };

            ctx.exit_interpret.store(match &ctx.next {
                Some(next) => next.clone().into_dyn_fift_value(),
                None => NopCont::value_instance(),
            });

            ctx.next = SeqCont::make(Some(this), ctx.next.take());
            break Ok(Some(compile_exec));
        }
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<text interpreter continuation>")
    }
}

#[derive(Clone, Copy)]
struct CompileExecuteCont;

impl FiftCont for CompileExecuteCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        Ok(if ctx.state.is_compile() {
            ctx.compile_stack_top()?;
            None
        } else {
            Some(ctx.execute_stack_top()?)
        })
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<compile execute continuation>")
    }
}

#[derive(Clone)]
pub struct ListCont {
    pub list: Rc<WordList>,
    pub after: Option<RcFiftCont>,
    pub pos: usize,
}

impl FiftCont for ListCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let is_last = self.pos + 1 >= self.list.items.len();
        let Some(current) = self.list.items.get(self.pos).cloned() else {
            return Ok(ctx.next.take());
        };

        match Rc::get_mut(&mut self) {
            Some(this) => {
                ctx.insert_before_next(&mut this.after);
                this.pos += 1;
                ctx.next = if is_last {
                    this.after.take()
                } else {
                    Some(self.into_dyn_fift_cont())
                };
            }
            None => {
                if let Some(next) = ctx.next.take() {
                    ctx.next = Some(RcFiftCont::new_dyn_fift_cont(ListCont {
                        after: SeqCont::make(self.after.clone(), Some(next)),
                        list: self.list.clone(),
                        pos: self.pos + 1,
                    }))
                } else if is_last {
                    ctx.next = self.after.clone()
                } else {
                    ctx.next = Some(RcFiftCont::new_dyn_fift_cont(ListCont {
                        after: self.after.clone(),
                        list: self.list.clone(),
                        pos: self.pos + 1,
                    }))
                }
            }
        }

        Ok(Some(current))
    }

    fn up(&self) -> Option<&RcFiftCont> {
        self.after.as_ref()
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_cont_name(self, d, f)
    }

    fn fmt_dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.pos == 0 {
            f.write_str("{")?;
            for item in &self.list.items {
                write!(f, " {}", item.display_name(d))?;
            }
            f.write_str(" }")
        } else {
            const N: usize = 16;

            if let Some(name) = d.resolve_name(self) {
                write!(f, "[in {name}:] ")?;
            }

            let len = self.list.items.len();
            let start = self.pos.saturating_sub(N);
            let items = self.list.items.iter();

            if start > 0 {
                f.write_str("... ")?;
            }
            for (i, item) in items.enumerate().skip(start).take(N) {
                if i == self.pos {
                    f.write_str("**HERE** ")?;
                }
                write!(f, "{} ", item.display_name(d))?;
            }
            if self.pos + N < len {
                f.write_str("...")?;
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
pub struct NopCont;

impl NopCont {
    thread_local! {
        static INSTANCE: (RcFiftCont, SafeRc<dyn StackValue>) = {
            let cont = RcFiftCont::new_dyn_fift_cont(NopCont);
            let value: SafeRc<dyn StackValue> = cont.clone().into_dyn_fift_value();
            (cont, value)
        };
    }

    pub fn instance() -> RcFiftCont {
        Self::INSTANCE.with(|(c, _)| c.clone())
    }

    pub fn value_instance() -> SafeRc<dyn StackValue> {
        Self::INSTANCE.with(|(_, v)| v.clone())
    }

    pub fn is_nop(cont: &dyn FiftCont) -> bool {
        let left = Self::INSTANCE.with(|(c, _)| SafeRc::as_ptr(c) as *const ());
        std::ptr::addr_eq(left, cont)
    }
}

impl FiftCont for NopCont {
    fn run(self: Rc<Self>, _: &mut crate::Context) -> Result<Option<RcFiftCont>> {
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<nop>")
    }
}

#[derive(Clone)]
pub struct SeqCont {
    pub first: Option<RcFiftCont>,
    pub second: Option<RcFiftCont>,
}

impl SeqCont {
    pub fn make(first: Option<RcFiftCont>, second: Option<RcFiftCont>) -> Option<RcFiftCont> {
        if second.is_none() {
            first
        } else if let Some(first) = first {
            Some(RcFiftCont::new_dyn_fift_cont(Self {
                first: Some(first),
                second,
            }))
        } else {
            second
        }
    }
}

impl FiftCont for SeqCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        Ok(match Rc::get_mut(&mut self) {
            Some(this) => {
                if ctx.next.is_none() {
                    ctx.next = this.second.take();
                    this.first.take()
                } else {
                    let result = std::mem::replace(&mut this.first, this.second.take());
                    this.second = ctx.next.take();
                    ctx.next = Some(self.into_dyn_fift_cont());
                    result
                }
            }
            None => {
                ctx.next = SeqCont::make(self.second.clone(), ctx.next.take());
                self.first.clone()
            }
        })
    }

    fn up(&self) -> Option<&RcFiftCont> {
        self.second.as_ref()
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(first) = &self.first {
            first.as_ref().fmt_name(d, f)
        } else {
            Ok(())
        }
    }

    fn fmt_dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(first) = &self.first {
            first.as_ref().fmt_dump(d, f)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct TimesCont {
    pub body: Option<RcFiftCont>,
    pub after: Option<RcFiftCont>,
    pub count: usize,
}

impl FiftCont for TimesCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        Ok(match Rc::get_mut(&mut self) {
            Some(this) => {
                ctx.insert_before_next(&mut this.after);

                if this.count > 1 {
                    this.count -= 1;
                    let body = this.body.clone();
                    ctx.next = Some(self.into_dyn_fift_cont());
                    body
                } else {
                    ctx.next = this.after.take();
                    this.body.take()
                }
            }
            None => {
                let next = SeqCont::make(self.after.clone(), ctx.next.take());

                ctx.next = if self.count > 1 {
                    Some(RcFiftCont::new_dyn_fift_cont(Self {
                        body: self.body.clone(),
                        after: next,
                        count: self.count - 1,
                    }))
                } else {
                    next
                };

                self.body.clone()
            }
        })
    }

    fn up(&self) -> Option<&RcFiftCont> {
        self.after.as_ref()
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<repeat {} times>", self.count)
    }

    fn fmt_dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<repeat {} times:> ", self.count)?;
        if let Some(body) = &self.body {
            FiftCont::fmt_dump(body.as_ref(), d, f)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct UntilCont {
    pub body: Option<RcFiftCont>,
    pub after: Option<RcFiftCont>,
}

impl FiftCont for UntilCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        if ctx.stack.pop_bool()? {
            return Ok(match Rc::get_mut(&mut self) {
                Some(this) => this.after.take(),
                None => self.after.clone(),
            });
        }

        let body = self.body.clone();
        let next = match Rc::get_mut(&mut self) {
            Some(this) => {
                ctx.insert_before_next(&mut this.after);
                self
            }
            None => {
                if let Some(next) = ctx.next.take() {
                    Rc::new(UntilCont {
                        body: self.body.clone(),
                        after: SeqCont::make(self.after.clone(), Some(next)),
                    })
                } else {
                    self
                }
            }
        };
        ctx.next = Some(next.into_dyn_fift_cont());
        Ok(body)
    }

    fn up(&self) -> Option<&RcFiftCont> {
        self.after.as_ref()
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<until loop continuation>")
    }

    fn fmt_dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<until loop continuation:> ")?;
        if let Some(body) = &self.body {
            FiftCont::fmt_dump(body.as_ref(), d, f)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct WhileCont {
    pub condition: Option<RcFiftCont>,
    pub body: Option<RcFiftCont>,
    pub after: Option<RcFiftCont>,
    pub running_body: bool,
}

impl WhileCont {
    fn stage_name(&self) -> &'static str {
        if self.running_body {
            "body"
        } else {
            "condition"
        }
    }
}

impl FiftCont for WhileCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let cont = if self.running_body {
            if !ctx.stack.pop_bool()? {
                return Ok(match Rc::get_mut(&mut self) {
                    Some(this) => this.after.take(),
                    None => self.after.clone(),
                });
            }

            self.body.clone()
        } else {
            self.condition.clone()
        };

        let next = match Rc::get_mut(&mut self) {
            Some(this) => {
                ctx.insert_before_next(&mut this.after);
                this.running_body = !this.running_body;
                self
            }
            None => Rc::new(Self {
                condition: self.condition.clone(),
                body: self.body.clone(),
                after: SeqCont::make(self.after.clone(), ctx.next.take()),
                running_body: !self.running_body,
            }),
        };

        ctx.next = Some(next.into_dyn_fift_cont());
        Ok(cont)
    }

    fn up(&self) -> Option<&RcFiftCont> {
        self.after.as_ref()
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<while loop {}>", self.stage_name())
    }

    fn fmt_dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<while loop {}:>", self.stage_name())?;
        let stage = if self.running_body {
            self.body.as_ref()
        } else {
            self.condition.as_ref()
        };
        if let Some(stage) = stage {
            FiftCont::fmt_dump(stage.as_ref(), d, f)?;
        }
        Ok(())
    }
}

pub struct LoopCont<T> {
    inner: T,
    state: LoopContState,
    func: RcFiftCont,
    after: Option<RcFiftCont>,
}

impl<T: Clone> Clone for LoopCont<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            state: self.state,
            func: self.func.clone(),
            after: self.after.clone(),
        }
    }
}

impl<T> LoopCont<T> {
    pub fn new(inner: T, func: RcFiftCont, after: Option<RcFiftCont>) -> Self {
        Self {
            inner,
            state: LoopContState::Init,
            func,
            after,
        }
    }
}

impl<T: LoopContImpl + 'static> FiftCont for LoopCont<T> {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let Some(this) = Rc::get_mut(&mut self) else {
            return Ok(Some(SafeRc::new_dyn_fift_cont(Self {
                inner: self.inner.clone(),
                state: self.state,
                func: self.func.clone(),
                after: self.after.clone(),
            })));
        };

        ctx.insert_before_next(&mut this.after);
        Ok(loop {
            match this.state {
                LoopContState::Init => {
                    if !this.inner.init(ctx)? {
                        break this.after.take();
                    }
                    this.state = LoopContState::PreExec;
                }
                LoopContState::PreExec => {
                    if !this.inner.pre_exec(ctx)? {
                        this.state = LoopContState::Finalize;
                        continue;
                    }
                    this.state = LoopContState::PostExec;
                    let res = self.func.clone();
                    ctx.next = Some(self.into_dyn_fift_cont());
                    break Some(res);
                }
                LoopContState::PostExec => {
                    if !this.inner.post_exec(ctx)? {
                        this.state = LoopContState::Finalize;
                        continue;
                    }
                    this.state = LoopContState::PreExec;
                    break Some(self.into_dyn_fift_cont());
                }
                LoopContState::Finalize => {
                    break if this.inner.finalize(ctx)? {
                        this.after.take()
                    } else {
                        None
                    };
                }
            }
        })
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<generic loop continuation state {:?}>", self.state)
    }
}

pub trait LoopContImpl: Clone {
    fn init(&mut self, ctx: &mut Context) -> Result<bool> {
        _ = ctx;
        Ok(true)
    }
    fn pre_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        _ = ctx;
        Ok(true)
    }
    fn post_exec(&mut self, ctx: &mut Context) -> Result<bool> {
        _ = ctx;
        Ok(true)
    }
    fn finalize(&mut self, ctx: &mut Context) -> Result<bool> {
        _ = ctx;
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum LoopContState {
    Init,
    PreExec,
    PostExec,
    Finalize,
}

#[derive(Clone)]
pub struct IntLitCont(BigInt);

impl<T> From<T> for IntLitCont
where
    BigInt: From<T>,
{
    fn from(value: T) -> Self {
        Self(BigInt::from(value))
    }
}

impl FiftCont for IntLitCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let value = match Rc::try_unwrap(self) {
            Ok(value) => value.0,
            Err(this) => this.0.clone(),
        };
        ctx.stack.push(value)?;
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone)]
pub struct LitCont(pub SafeRc<dyn StackValue>);

impl FiftCont for LitCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        let value = match Rc::try_unwrap(self) {
            Ok(value) => value.0,
            Err(this) => this.0.clone(),
        };
        ctx.stack.push_raw(value)?;
        Ok(None)
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_lit_cont_name(self.0.as_ref(), d, f)
    }
}

#[derive(Clone)]
pub struct MultiLitCont(pub Vec<SafeRc<dyn StackValue>>);

impl FiftCont for MultiLitCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        match Rc::try_unwrap(self) {
            Ok(value) => {
                for item in value.0 {
                    ctx.stack.push_raw(item)?;
                }
            }
            Err(this) => {
                for item in &this.0 {
                    ctx.stack.push_raw(item.clone())?;
                }
            }
        };
        Ok(None)
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for item in &self.0 {
            if !std::mem::take(&mut first) {
                f.write_str(" ")?;
            }
            write_lit_cont_name(item.as_ref(), d, f)?;
        }
        Ok(())
    }
}

pub type ContextWordFunc = fn(&mut Context) -> Result<()>;

impl FiftCont for ContextWordFunc {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        (self)(ctx)?;
        Ok(None)
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_cont_name(self, d, f)
    }
}

pub type ContextTailWordFunc = fn(&mut Context) -> Result<Option<RcFiftCont>>;

impl FiftCont for ContextTailWordFunc {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        (self)(ctx)
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_cont_name(self, d, f)
    }
}

pub type StackWordFunc = fn(&mut Stack) -> Result<()>;

impl FiftCont for StackWordFunc {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
        (self)(&mut ctx.stack)?;
        Ok(None)
    }

    fn fmt_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_cont_name(self, d, f)
    }
}

// === impl Context ===

impl Context<'_> {
    fn insert_before_next(&mut self, cont: &mut Option<RcFiftCont>) {
        if let Some(next) = self.next.take() {
            *cont = match cont.take() {
                Some(prev) => Some(RcFiftCont::new_dyn_fift_cont(SeqCont {
                    first: Some(prev),
                    second: Some(next),
                })),
                None => Some(next),
            };
        }
    }
}

fn write_lit_cont_name(
    stack_entry: &dyn StackValue,
    d: &Dictionary,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    let ty = stack_entry.ty();
    match ty {
        StackValueType::Int | StackValueType::String | StackValueType::Builder => {
            stack_entry.fmt_dump(f)
        }
        _ => {
            if let Ok(cont) = stack_entry.as_cont() {
                write!(f, "{{ {} }}", cont.display_dump(d))
            } else {
                write!(f, "<literal of type {:?}>", ty)
            }
        }
    }
}

fn write_cont_name(
    cont: &dyn FiftCont,
    d: &Dictionary,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    if let Some(name) = d.resolve_name(cont) {
        f.write_str(name.trim_end())
    } else {
        write!(f, "<continuation {:?}>", cont as *const dyn FiftCont)
    }
}
