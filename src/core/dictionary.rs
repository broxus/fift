use std::collections::hash_map::{self, HashMap};
use std::rc::Rc;

use super::cont::{Cont, ContImpl, ContextTailWordFunc, ContextWordFunc, StackWordFunc};
use crate::error::*;

pub struct DictionaryEntry {
    pub definition: Cont,
    pub active: bool,
}

impl DictionaryEntry {
    pub fn new_ordinary(definition: Cont) -> Self {
        Self {
            definition,
            active: false,
        }
    }

    pub fn new_active(definition: Cont) -> Self {
        Self {
            definition,
            active: true,
        }
    }
}

pub struct Dictionary {
    words: WordsMap,
    nop: Cont,
}

impl Default for Dictionary {
    fn default() -> Self {
        struct NopCont;

        impl ContImpl for NopCont {
            fn run(self: Rc<Self>, _: &mut crate::Context) -> Result<Option<Cont>> {
                Ok(None)
            }

            fn write_name(
                &self,
                _: &Dictionary,
                f: &mut std::fmt::Formatter<'_>,
            ) -> std::fmt::Result {
                f.write_str("<nop>")
            }
        }

        Self {
            words: Default::default(),
            nop: Rc::new(NopCont),
        }
    }
}

impl Dictionary {
    pub fn make_nop(&self) -> Cont {
        self.nop.clone()
    }

    pub fn is_nop(&self, cont: &dyn ContImpl) -> bool {
        let left = Rc::as_ptr(&self.nop) as *const ();
        let right = cont as *const _ as *const ();
        std::ptr::eq(left, right)
    }

    pub fn lookup(&self, name: &str) -> Option<&DictionaryEntry> {
        self.words.get(name)
    }

    pub fn resolve_name(&self, definition: &dyn ContImpl) -> Option<&str> {
        for (name, entry) in &self.words {
            // NOTE: erase trait data from fat pointers
            let left = Rc::as_ptr(&entry.definition) as *const ();
            let right = definition as *const _ as *const ();
            // Compare only the address part
            if std::ptr::eq(left, right) {
                return Some(name);
            }
        }
        None
    }

    pub fn define_context_word<T: Into<String>>(
        &mut self,
        name: T,
        f: ContextWordFunc,
    ) -> Result<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: false,
            },
            false,
        )
    }

    pub fn define_context_tail_word<T: Into<String>>(
        &mut self,
        name: T,
        f: ContextTailWordFunc,
    ) -> Result<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: false,
            },
            false,
        )
    }

    pub fn define_active_word<T: Into<String>>(
        &mut self,
        name: T,
        f: ContextWordFunc,
    ) -> Result<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: true,
            },
            false,
        )
    }

    pub fn define_stack_word<T: Into<String>>(&mut self, name: T, f: StackWordFunc) -> Result<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: false,
            },
            false,
        )
    }

    pub fn define_word<T: Into<String>>(
        &mut self,
        name: T,
        word: DictionaryEntry,
        allow_redefine: bool,
    ) -> Result<()> {
        fn define_word_impl(
            words: &mut WordsMap,
            name: String,
            word: DictionaryEntry,
            allow_redefine: bool,
        ) -> Result<()> {
            match words.entry(name) {
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(word);
                    Ok(())
                }
                hash_map::Entry::Occupied(mut entry) if allow_redefine => {
                    entry.insert(word);
                    Ok(())
                }
                _ => Err(Error::TypeRedefenition),
            }
        }
        define_word_impl(&mut self.words, name.into(), word, allow_redefine)
    }
}

type WordsMap = HashMap<String, DictionaryEntry>;
