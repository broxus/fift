extern crate self as fift;

pub use self::core::Context;

pub mod core;
pub mod error;
pub mod modules;
pub mod util;

impl Context<'_> {
    pub fn with_basic_modules(self) -> error::Result<Self> {
        use modules::*;
        self.with_module(BaseModule)?
            .with_module(Arithmetic)?
            .with_module(CellUtils)?
            .with_module(Control)?
            .with_module(DebugUtils)?
            .with_module(StackUtils)?
            .with_module(StringUtils)?
            .with_module(Crypto)
    }
}
