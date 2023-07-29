use std::collections::hash_map::{self, HashMap};
use std::rc::Rc;

use crate::continuation::*;
use crate::error::*;
use crate::stack::*;

pub struct DictionaryEntry {
    pub definition: Continuation,
    pub active: bool,
}

impl DictionaryEntry {
    pub fn new_ordinary(definition: Continuation) -> Self {
        Self {
            definition,
            active: false,
        }
    }

    pub fn new_active(definition: Continuation) -> Self {
        Self {
            definition,
            active: true,
        }
    }
}

pub struct Dictionary {
    words: WordsMap,
    nop: Continuation,
}

impl Default for Dictionary {
    fn default() -> Self {
        fn interpret_nop(_: &mut Stack) -> FiftResult<()> {
            Ok(())
        }

        Self {
            words: Default::default(),
            nop: Rc::new(interpret_nop as StackWordFunc),
        }
    }
}

impl Dictionary {
    pub fn make_nop(&self) -> Continuation {
        self.nop.clone()
    }

    pub fn lookup(&self, name: &str) -> Option<&DictionaryEntry> {
        self.words.get(name)
    }

    pub fn resolve_name(&self, definition: &dyn ContinuationImpl) -> Option<&str> {
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
    ) -> FiftResult<()> {
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
    ) -> FiftResult<()> {
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
    ) -> FiftResult<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: true,
            },
        )
    }

    pub fn define_stack_word<T: Into<String>>(
        &mut self,
        name: T,
        f: StackWordFunc,
    ) -> FiftResult<()> {
        self.define_word(
            name,
            DictionaryEntry {
                definition: Rc::new(f),
                active: false,
            },
        )
    }

    pub fn define_word<T: Into<String>>(
        &mut self,
        name: T,
        word: DictionaryEntry,
    ) -> FiftResult<()> {
        fn define_word_impl(
            words: &mut WordsMap,
            name: String,
            word: DictionaryEntry,
        ) -> FiftResult<()> {
            match words.entry(name) {
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(word);
                    Ok(())
                }
                hash_map::Entry::Occupied(_) => Err(FiftError::TypeRedefenition),
            }
        }
        define_word_impl(&mut self.words, name.into(), word)
    }
}

type WordsMap = HashMap<String, DictionaryEntry>;
