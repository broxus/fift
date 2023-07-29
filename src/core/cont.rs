use std::rc::Rc;

use num_bigint::BigInt;

use super::{Context, Dictionary, Stack};

use crate::error::*;

pub type Cont = Rc<dyn ContImpl>;

pub trait ContImpl {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>>;

    fn up(&self) -> Option<&Cont> {
        None
    }

    fn write_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;

    fn dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_name(d, f)
    }
}

impl dyn ContImpl + '_ {
    pub fn display_backtrace<'a>(&'a self, d: &'a Dictionary) -> impl std::fmt::Display + 'a {
        struct ContinuationBacktrace<'a> {
            d: &'a Dictionary,
            cont: &'a dyn ContImpl,
        }

        impl std::fmt::Display for ContinuationBacktrace<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut cont = self.cont;
                let mut newline = "";
                for i in 1..=16 {
                    write!(f, "{newline}level {i}: {}", cont.display_dump(self.d))?;
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
            cont: &'a dyn ContImpl,
        }

        impl std::fmt::Display for ContinuationWriteName<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.cont.write_name(self.d, f)
            }
        }

        ContinuationWriteName { d, cont: self }
    }

    pub fn display_dump<'a>(&'a self, d: &'a Dictionary) -> impl std::fmt::Display + 'a {
        struct ContinuationDump<'a> {
            d: &'a Dictionary,
            cont: &'a dyn ContImpl,
        }

        impl std::fmt::Display for ContinuationDump<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.cont.dump(self.d, f)
            }
        }

        ContinuationDump { d, cont: self }
    }
}

pub struct SeqCont {
    pub first: Option<Cont>,
    pub second: Option<Cont>,
}

impl SeqCont {
    pub fn make(first: Option<Cont>, second: Option<Cont>) -> Option<Cont> {
        if second.is_none() {
            first
        } else {
            Some(Rc::new(Self { first, second }))
        }
    }
}

impl ContImpl for SeqCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
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

    fn up(&self) -> Option<&Cont> {
        self.second.as_ref()
    }

    fn write_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("seq: ")?;
        if let Some(first) = &self.first {
            first.as_ref().write_name(d, f)
        } else {
            Ok(())
        }
    }

    fn dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("seq: ")?;
        if let Some(first) = &self.first {
            first.as_ref().dump(d, f)?;
        }
        Ok(())
    }
}

pub struct TimesCont {
    pub body: Option<Cont>,
    pub after: Option<Cont>,
    pub count: usize,
}

impl ContImpl for TimesCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
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

    fn up(&self) -> Option<&Cont> {
        self.after.as_ref()
    }

    fn write_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<repeat {} times>", self.count)
    }

    fn dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<repeat {} times:> ", self.count)?;
        if let Some(body) = &self.body {
            ContImpl::dump(body.as_ref(), d, f)?;
        }
        Ok(())
    }
}

pub struct UntilCont {
    pub body: Option<Cont>,
    pub after: Option<Cont>,
}

impl ContImpl for UntilCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
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

    fn up(&self) -> Option<&Cont> {
        self.after.as_ref()
    }

    fn write_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<until loop continuation>")
    }

    fn dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<until loop continuation:> ")?;
        if let Some(body) = &self.body {
            ContImpl::dump(body.as_ref(), d, f)?;
        }
        Ok(())
    }
}

pub struct WhileCont {
    pub condition: Option<Cont>,
    pub body: Option<Cont>,
    pub after: Option<Cont>,
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

impl ContImpl for WhileCont {
    fn run(mut self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
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

    fn up(&self) -> Option<&Cont> {
        self.after.as_ref()
    }

    fn write_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<while loop {}>", self.stage_name())
    }

    fn dump(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<while loop {}:>", self.stage_name())?;
        let stage = if self.running_body {
            self.body.as_ref()
        } else {
            self.condition.as_ref()
        };
        if let Some(stage) = stage {
            ContImpl::dump(stage.as_ref(), d, f)?;
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

impl ContImpl for IntLitCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        let value = match Rc::try_unwrap(self) {
            Ok(value) => value.0,
            Err(this) => this.0.clone(),
        };
        ctx.stack.push(value)?;
        Ok(None)
    }

    fn write_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type ContextWordFunc = fn(&mut Context) -> Result<()>;

impl ContImpl for ContextWordFunc {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        (self)(ctx)?;
        Ok(None)
    }

    fn write_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print_cont_name(self, d, f)
    }
}

pub type ContextTailWordFunc = fn(&mut Context) -> Result<Option<Cont>>;

impl ContImpl for ContextTailWordFunc {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        (self)(ctx)
    }

    fn write_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print_cont_name(self, d, f)
    }
}

pub type StackWordFunc = fn(&mut Stack) -> Result<()>;

impl ContImpl for StackWordFunc {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        (self)(&mut ctx.stack)?;
        Ok(None)
    }

    fn write_name(&self, d: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print_cont_name(self, d, f)
    }
}

/// === impl Context ===

impl Context<'_> {
    fn insert_before_next(&mut self, cont: &mut Option<Cont>) {
        if let Some(next) = self.next.take() {
            *cont = Some(Rc::new(SeqCont {
                first: cont.take(),
                second: Some(next),
            }));
        }
    }
}

fn print_cont_name(
    cont: &dyn ContImpl,
    d: &Dictionary,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    if let Some(name) = d.resolve_name(cont) {
        f.write_str(name.trim_end())
    } else {
        write!(f, "<continuation {:?}>", cont as *const dyn ContImpl)
    }
}
