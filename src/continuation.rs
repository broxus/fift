use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::context::*;
use crate::error::*;
use crate::stack::*;

pub type Continuation = Rc<dyn ContinuationImpl>;

pub trait ContinuationImpl {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>>;

    fn up(&self) -> Option<&Continuation> {
        None
    }

    fn write_name(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;

    fn dump(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_name(ctx, f)
    }
}

impl dyn ContinuationImpl {
    pub fn display_dump<'a: 'b, 'b>(&'a self, ctx: &'a Context<'b>) -> impl std::fmt::Display + 'a {
        struct DumpContinuation<'a, 'b> {
            ctx: &'a Context<'b>,
            cont: &'a dyn ContinuationImpl,
        }

        impl std::fmt::Display for DumpContinuation<'_, '_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.cont.dump(self.ctx, f)
            }
        }

        DumpContinuation { ctx, cont: self }
    }
}

pub struct InterpretCont;

impl ContinuationImpl for InterpretCont {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        thread_local! {
            static COMPILE_EXECUTE: Continuation = Rc::new(CompileExecuteCont);
            static WORD: RefCell<String> = RefCell::new(String::with_capacity(128));
        };

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
                if let Some(value) = token.parse_number()? {
                    ctx.stack.push(Box::new(value.num))?;
                    if let Some(denom) = value.denom {
                        ctx.stack.push(Box::new(denom))?;
                        ctx.stack.push_argcount(2, ctx.dictionary.make_nop())?;
                    } else {
                        ctx.stack.push_argcount(1, ctx.dictionary.make_nop())?;
                    }
                    break 'token;
                }

                return Err(FiftError::UndefinedWord);
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

    fn up(&self) -> Option<&Continuation> {
        None
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<text interpreter continuation>")
    }
}

pub struct CompileExecuteCont;

impl ContinuationImpl for CompileExecuteCont {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        if ctx.state.is_compile() {
            ctx.interpret_compile()?;
            Ok(None)
        } else {
            ctx.interpret_execute()
        }
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<compile execute continuation>")
    }
}

pub struct QuitCont {
    pub exit_code: u8,
}

impl ContinuationImpl for QuitCont {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        ctx.exit_code = self.exit_code;
        ctx.next = None;
        Ok(None)
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<quit {}>", self.exit_code)
    }
}

pub struct SeqCont {
    pub first: Option<Continuation>,
    pub second: Option<Continuation>,
}

impl SeqCont {
    fn make(first: Option<Continuation>, second: Option<Continuation>) -> Option<Continuation> {
        if second.is_none() {
            first
        } else {
            Some(Rc::new(Self { first, second }))
        }
    }
}

impl ContinuationImpl for SeqCont {
    fn run_tail(mut self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        Ok(match Rc::get_mut(&mut self) {
            Some(this) => {
                if ctx.next.is_none() {
                    ctx.next = this.second.take();
                    this.first.take()
                } else {
                    let result = std::mem::replace(&mut this.first, this.second.take());
                    this.second = ctx.next.take();
                    ctx.next = Some(self);
                    result
                }
            }
            None => {
                ctx.next = SeqCont::make(self.second.clone(), ctx.next.take());
                self.first.clone()
            }
        })
    }

    fn up(&self) -> Option<&Continuation> {
        self.second.as_ref()
    }

    fn write_name(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("seq: ")?;
        if let Some(first) = &self.first {
            first.as_ref().write_name(ctx, f)
        } else {
            Ok(())
        }
    }

    fn dump(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("seq: ")?;
        if let Some(first) = &self.first {
            first.as_ref().dump(ctx, f)?;
        }
        Ok(())
    }
}

pub struct TimesCont {
    pub body: Option<Continuation>,
    pub after: Option<Continuation>,
    pub count: usize,
}

impl ContinuationImpl for TimesCont {
    fn run_tail(mut self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        Ok(match Rc::get_mut(&mut self) {
            Some(this) => {
                ctx.insert_before_next(&mut this.after);

                if this.count > 1 {
                    this.count -= 1;
                    let body = this.body.clone();
                    ctx.next = Some(self);
                    body
                } else {
                    ctx.next = this.after.take();
                    this.body.take()
                }
            }
            None => {
                let next = SeqCont::make(self.after.clone(), ctx.next.take());

                ctx.next = if self.count > 1 {
                    Some(Rc::new(Self {
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

    fn up(&self) -> Option<&Continuation> {
        self.after.as_ref()
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<repeat {} times>", self.count)
    }

    fn dump(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<repeat {} times:> ", self.count)?;
        if let Some(body) = &self.body {
            ContinuationImpl::dump(body.as_ref(), ctx, f)?;
        }
        Ok(())
    }
}

pub struct UntilCont {
    pub body: Option<Continuation>,
    pub after: Option<Continuation>,
}

impl ContinuationImpl for UntilCont {
    fn run_tail(mut self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
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
        ctx.next = Some(next);
        Ok(body)
    }

    fn up(&self) -> Option<&Continuation> {
        self.after.as_ref()
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<until loop continuation>")
    }

    fn dump(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<until loop continuation:> ")?;
        if let Some(body) = &self.body {
            ContinuationImpl::dump(body.as_ref(), ctx, f)?;
        }
        Ok(())
    }
}

pub struct WhileCont {
    pub condition: Option<Continuation>,
    pub body: Option<Continuation>,
    pub after: Option<Continuation>,
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

impl ContinuationImpl for WhileCont {
    fn run_tail(mut self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
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

        ctx.next = Some(next);
        Ok(cont)
    }

    fn up(&self) -> Option<&Continuation> {
        self.after.as_ref()
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<while loop {}>", self.stage_name())
    }

    fn dump(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<while loop {}:>", self.stage_name())?;
        let stage = if self.running_body {
            self.body.as_ref()
        } else {
            self.condition.as_ref()
        };
        if let Some(stage) = stage {
            ContinuationImpl::dump(stage.as_ref(), ctx, f)?;
        }
        Ok(())
    }
}

pub struct IntLitCont(BigInt);

impl From<i32> for IntLitCont {
    fn from(value: i32) -> Self {
        Self(BigInt::from(value))
    }
}

impl ContinuationImpl for IntLitCont {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        let value = match Rc::try_unwrap(self) {
            Ok(value) => value.0,
            Err(this) => this.0.clone(),
        };
        ctx.stack.push(Box::new(value))?;
        Ok(None)
    }

    fn write_name(&self, _: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type ContextWordFunc = fn(&mut Context) -> FiftResult<()>;

impl ContinuationImpl for ContextWordFunc {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        (self)(ctx)?;
        Ok(None)
    }

    fn write_name(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print_cont_name(self, ctx, f)
    }
}

pub type ContextTailWordFunc = fn(&mut Context) -> FiftResult<Option<Continuation>>;

impl ContinuationImpl for ContextTailWordFunc {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        (self)(ctx)
    }

    fn write_name(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print_cont_name(self, ctx, f)
    }
}

pub type StackWordFunc = fn(&mut Stack) -> FiftResult<()>;

impl ContinuationImpl for StackWordFunc {
    fn run_tail(self: Rc<Self>, ctx: &mut Context) -> FiftResult<Option<Continuation>> {
        (self)(&mut ctx.stack)?;
        Ok(None)
    }

    fn write_name(&self, ctx: &Context, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print_cont_name(self, ctx, f)
    }
}

/// === impl Context ===

impl Context<'_> {
    fn insert_before_next(&mut self, cont: &mut Option<Continuation>) {
        if let Some(next) = self.next.take() {
            *cont = Some(Rc::new(SeqCont {
                first: cont.take(),
                second: Some(next),
            }));
        }
    }
}

fn print_cont_name(
    cont: &dyn ContinuationImpl,
    ctx: &Context,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    if let Some(name) = ctx.dictionary.resolve_name(cont) {
        f.write_str(name.trim_end())
    } else {
        write!(
            f,
            "<continuation {:?}>",
            cont as *const dyn ContinuationImpl
        )
    }
}
