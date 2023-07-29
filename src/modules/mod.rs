use crate::core::*;
use crate::error::*;

pub use self::arithmetic::Arithmetic;
pub use self::cell_utils::CellUtils;
pub use self::control::Control;
pub use self::debug_utils::DebugUtils;
pub use self::stack_utils::StackUtils;
pub use self::string_utils::StringUtils;

mod arithmetic;
mod cell_utils;
mod control;
mod debug_utils;
mod stack_utils;
mod string_utils;

pub struct BaseModule;

#[fift_module]
impl FiftModule for BaseModule {
    #[init]
    fn init(d: &mut Dictionary) -> Result<()> {
        d.define_word("nop", DictionaryEntry::new_ordinary(d.make_nop()))
    }
}
