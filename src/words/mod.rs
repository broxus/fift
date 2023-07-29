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

mod common;
