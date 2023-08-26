use std::rc::Rc;

use anyhow::Result;

use crate::core::*;

pub struct VmUtils;

#[fift_module]
impl VmUtils {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        thread_local! {
            static VM_LIBRARIES: Rc<dyn StackValue> = Rc::new(SharedBox::default());
        }

        let vm_libraries = VM_LIBRARIES.with(|b| b.clone());

        d.define_word("vmlibs ", Rc::new(cont::LitCont(vm_libraries)))?;

        Ok(())
    }

    #[cmd(name = "runvmx")]
    #[cmd(name = "dbrunvm")]
    #[cmd(name = "dbrunvm-parallel")]
    #[cmd(name = "vmcont")]
    #[cmd(name = "vmcont@")]
    #[cmd(name = "(vmoplen)")]
    #[cmd(name = "(vmopdump)")]
    fn interpret_run_vm(_ctx: &mut Context) -> Result<()> {
        anyhow::bail!("Unimplemented");
    }
}
