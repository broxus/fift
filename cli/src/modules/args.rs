use std::rc::Rc;

use anyhow::Result;
use fift::core::*;
use tycho_vm::SafeRc;

pub struct CmdArgsUtils {
    name: SafeRc<dyn StackValue>,
    args: Vec<SafeRc<dyn StackValue>>,
}

impl CmdArgsUtils {
    pub fn new(args: Vec<String>) -> Self {
        let mut args = args.into_iter();

        let name = SafeRc::new_dyn_fift_value(args.next().unwrap_or_default());

        let args = args.map(SafeRc::new_dyn_fift_value).collect::<Vec<_>>();

        Self { name, args }
    }
}

#[fift_module]
impl CmdArgsUtils {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        d.define_word(
            "$0 ",
            RcFiftCont::new_dyn_fift_cont(cont::LitCont(self.name.clone())),
        )?;

        let mut list = Stack::make_null();
        for (i, arg) in self.args.iter().enumerate().rev() {
            list = cons(arg.clone(), list);
            d.define_word(
                format!("${} ", i + 1),
                RcFiftCont::new_dyn_fift_cont(cont::LitCont(arg.clone())),
            )?;
        }

        d.define_word(
            "$# ",
            RcFiftCont::new_dyn_fift_cont(cont::IntLitCont::from(self.args.len())),
        )?;

        let mut all_args = Vec::with_capacity(1 + self.args.len());
        all_args.push(self.name.clone());
        all_args.extend_from_slice(&self.args);
        d.define_word("$() ", RcFiftCont::new_dyn_fift_cont(CmdArgCont(all_args)))?;

        d.define_word(
            "$* ",
            RcFiftCont::new_dyn_fift_cont(cont::LitCont(SafeRc::new_dyn_fift_value(SharedBox::new(
                list,
            )))),
        )?;

        Ok(())
    }
}

struct CmdArgCont(Vec<SafeRc<dyn StackValue>>);

impl FiftCont for CmdArgCont {
    fn run(self: Rc<Self>, ctx: &mut Context) -> Result<Option<RcFiftCont>> {
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

fn cons(head: SafeRc<dyn StackValue>, tail: SafeRc<dyn StackValue>) -> SafeRc<dyn StackValue> {
    SafeRc::new_dyn_fift_value(vec![head, tail])
}
