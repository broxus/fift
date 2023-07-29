use crate::context::*;
use crate::dictionary::*;
use crate::error::*;

macro_rules! words {
    ($d:ident, {
        $(@$t:tt $name:literal => $expr:expr),*$(,)?
    }) => {
        $(words!(@$t, $d, $name, $expr)?;)*
    };

    (@raw, $d:ident, $lit:literal, $expr:expr) => {
        $d.define_word(concat!($lit, " "), $expr)
    };
    (@ctx, $d:ident, $lit:literal, $expr:expr) => {
        $d.define_context_word(concat!($lit, " "), $expr)
    };
    (@ctl, $d:ident, $lit:literal, $expr:expr) => {
        $d.define_context_tail_word(concat!($lit, " "), $expr)
    };
    (@act, $d:ident, $lit:literal, $expr:expr) => {
        $d.define_active_word(concat!($lit, " "), $expr)
    };
    (@stk, $d:ident, $lit:literal, $expr:expr) => {
        $d.define_stack_word(concat!($lit, " "), $expr)
    };
}

mod arithmetic_words;
mod cell_words;
mod control_words;
mod debug_words;
mod stack_words;

impl Context<'_> {
    pub fn init_common_words(&mut self) -> FiftResult<()> {
        let d: &mut Dictionary = &mut self.dictionary;

        words!(d, {
            @raw "nop" => DictionaryEntry::new_ordinary(d.make_nop()),
        });

        debug_words::init(d)?;
        stack_words::init(d)?;
        arithmetic_words::init(d)?;
        cell_words::init(d)?;
        control_words::init(d)?;

        Ok(())
    }
}
