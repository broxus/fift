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
}
