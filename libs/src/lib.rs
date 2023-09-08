use std::collections::HashMap;
use std::sync::OnceLock;

/// Returns a pair of name and contents of the base Fift library.
pub fn base_lib() -> LibraryDefinition {
    def::fift()
}

/// Returns a map with all predefined libraries.
pub fn all() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut libraries = HashMap::with_capacity(LIBRARIES.len());
        for LibraryDefinition { name, content } in LIBRARIES {
            libraries.insert(*name, *content);
        }
        libraries
    })
}

pub struct LibraryDefinition {
    pub name: &'static str,
    pub content: &'static str,
}

macro_rules! define_libs {
    ($prefix:literal, [
        $($name:ident => $file:literal),*$(,)?
    ]) => {
        /// Raw libraries.
        pub mod def {
            $(/// Returns a content of a `
            #[doc = $file]
            /// ` library.
            pub const fn $name() -> crate::LibraryDefinition {
                crate::LibraryDefinition {
                    name: $file,
                    content: include_str!(concat!($prefix, $file)),
                }
            })*
        }

        const LIBRARIES: &[LibraryDefinition] = &[
            $(def::$name()),*
        ];
    };
}

define_libs!(
    "./",
    [
        asm => "Asm.fif",
        disasm => "Disasm.fif",
        color => "Color.fif",
        fift => "Fift.fif",
        fift_ext => "FiftExt.fif",
        lisp => "Lisp.fif",
        lists => "Lists.fif",
        stack => "Stack.fif",
        ton_util => "TonUtil.fif",
    ]
);
