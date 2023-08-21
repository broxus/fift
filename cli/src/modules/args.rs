use std::rc::Rc;

use anyhow::Result;

use fift::core::*;

pub struct CmdArgsUtils {
    name: Rc<dyn StackValue>,
    args: Vec<Rc<dyn StackValue>>,
}

impl CmdArgsUtils {
    pub fn new(args: Vec<String>) -> Self {
        let mut args = args.into_iter();

        let name = Rc::new(args.next().unwrap_or_default()) as Rc<dyn StackValue>;

        let args = args
            .map(|value| Rc::new(value.clone()) as Rc<dyn StackValue>)
            .collect::<Vec<_>>();

        Self { name, args }
    }
}

#[fift_module]
impl CmdArgsUtils {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        d.define_word("$0 ", Rc::new(cont::LitCont(self.name.clone())))?;

        let mut list = Stack::make_null();
        for (i, arg) in self.args.iter().enumerate().rev() {
            list = cons(arg.clone(), list);
            d.define_word(format!("${} ", i + 1), Rc::new(cont::LitCont(arg.clone())))?;
        }

        d.define_word("$# ", Rc::new(cont::IntLitCont::from(self.args.len())))?;

        let mut all_args = Vec::with_capacity(1 + self.args.len());
        all_args.push(self.name.clone());
        all_args.extend_from_slice(&self.args);
        d.define_word("$() ", Rc::new(CmdArgCont(all_args)))?;

        d.define_word("$* ", Rc::new(cont::LitCont(Rc::new(SharedBox::new(list)))))?;

        Ok(())
    }
}

struct CmdArgCont(Vec<Rc<dyn StackValue>>);

impl ContImpl for CmdArgCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<Cont>> {
        let n = ctx.stack.pop_smallint_range(0, 999999)? as usize;
        match self.0.get(n).cloned() {
            None => ctx.stack.push_null()?,
            Some(value) => ctx.stack.push_raw(value)?,
        }
        Ok(None)
    }

    fn fmt_name(&self, _: &Dictionary, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("$()")
    }
}

fn cons(head: Rc<dyn StackValue>, tail: Rc<dyn StackValue>) -> Rc<dyn StackValue> {
    Rc::new(vec![head, tail])
}
