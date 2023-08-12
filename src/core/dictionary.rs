use std::collections::hash_map::{self, HashMap};
use std::rc::Rc;

use anyhow::Result;

use super::cont::{Cont, ContImpl, ContextTailWordFunc, ContextWordFunc, StackWordFunc};

pub struct DictionaryEntry {
    pub definition: Cont,
    pub active: bool,
}

impl From<Cont> for DictionaryEntry {
    fn from(value: Cont) -> Self {
        Self {
            definition: value,
            active: false,
        }
    }
}

impl<T: ContImpl + 'static> From<Rc<T>> for DictionaryEntry {
    fn from(value: Rc<T>) -> Self {
        Self {
            definition: value,
            active: false,
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

            fn fmt_name(
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

    pub fn words(&self) -> impl Iterator<Item = &String> {
        self.words.keys()
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
        )
    }

    pub fn define_stack_word<T: Into<String>>(&mut self, name: T, f: StackWordFunc) -> Result<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: false,
            },
        )
    }

    pub fn define_word<T, E>(&mut self, name: T, word: E) -> Result<()>
    where
        T: Into<String>,
        E: Into<DictionaryEntry>,
    {
        self.define_word_ext(name, word, false)
    }

    pub fn define_word_ext<T, E>(&mut self, name: T, word: E, allow_redefine: bool) -> Result<()>
    where
        T: Into<String>,
        E: Into<DictionaryEntry>,
    {
        fn define_word_impl(
            words: &mut WordsMap,
            name: String,
            word: DictionaryEntry,
            allow_redefine: bool,
        ) -> Result<()> {
            match words.entry(name.clone()) {
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(word);
                    Ok(())
                }
                hash_map::Entry::Occupied(mut entry) if allow_redefine => {
                    entry.insert(word);
                    Ok(())
                }
                _ => anyhow::bail!("Word `{name}` unexpectedly redefined"),
            }
        }
        define_word_impl(&mut self.words, name.into(), word.into(), allow_redefine)
    }

    pub fn undefine_word(&mut self, name: &str) -> bool {
        self.words.remove(name).is_some()
    }
}

type WordsMap = HashMap<String, DictionaryEntry>;
