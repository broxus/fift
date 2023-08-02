use std::rc::Rc;

use anyhow::Result;

use crate::core::*;

pub struct VmUtils;

#[fift_module]
impl VmUtils {
    #[init]
    fn init(d: &mut Dictionary) -> Result<()> {
        thread_local! {
            static VM_LIBRARIES: SharedBox = SharedBox::new(Box::new(()));
        }

        let vm_libraries: Box<dyn StackValue> = Box::new(VM_LIBRARIES.with(|b| b.clone()));

        d.define_word(
            "vmlibs ",
            DictionaryEntry::new_ordinary(Rc::new(cont::LitCont(vm_libraries))),
            false,
        )?;

        Ok(())
    }
}
