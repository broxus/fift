use std::collections::hash_map::{self, HashMap};
use std::rc::Rc;

use super::cont::{Cont, ContImpl, ContextTailWordFunc, ContextWordFunc, StackWordFunc};
use super::stack::Stack;
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
        fn interpret_nop(_: &mut Stack) -> Result<()> {
            Ok(())
        }

        Self {
            words: Default::default(),
            nop: Rc::new(interpret_nop as StackWordFunc),
        }
    }
}

impl Dictionary {
    pub fn make_nop(&self) -> Cont {
        self.nop.clone()
    }

    pub fn lookup(&self, name: &str) -> Option<&DictionaryEntry> {
        self.words.get(name)
    }

    pub fn resolve_name(&self, definition: &dyn ContImpl) -> Option<&str> {
        for (name, entry) in &self.words {
            if Rc::as_ptr(&entry.definition) == definition {
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

    pub fn define_word<T: Into<String>>(&mut self, name: T, word: DictionaryEntry) -> Result<()> {
        fn define_word_impl(
            words: &mut WordsMap,
            name: String,
            word: DictionaryEntry,
        ) -> Result<()> {
            match words.entry(name) {
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(word);
                    Ok(())
                }
                hash_map::Entry::Occupied(_) => Err(Error::TypeRedefenition),
            }
        }
        define_word_impl(&mut self.words, name.into(), word)
    }
}

type WordsMap = HashMap<String, DictionaryEntry>;
