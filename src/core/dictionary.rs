use std::rc::Rc;

use anyhow::Result;

use super::cont::{Cont, ContImpl, ContextTailWordFunc, ContextWordFunc, StackWordFunc};
use super::stack::{HashMapTreeKey, HashMapTreeKeyRef, HashMapTreeNode, SharedBox, StackValue};
use super::StackValueType;

pub struct DictionaryEntry {
    pub definition: Cont,
    pub active: bool,
}

impl DictionaryEntry {
    fn try_from_value(value: &dyn StackValue) -> Option<Self> {
        let (cont, active) = Self::cont_from_value(value)?;
        Some(Self {
            definition: cont.clone(),
            active,
        })
    }

    fn cont_from_value(value: &dyn StackValue) -> Option<(&Cont, bool)> {
        if let Ok(cont) = value.as_cont() {
            return Some((cont, false));
        } else if let Ok(tuple) = value.as_tuple() {
            if tuple.len() == 1 {
                if let Ok(cont) = tuple.first()?.as_cont() {
                    return Some((cont, true));
                }
            }
        }
        None
    }
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

impl From<DictionaryEntry> for Rc<dyn StackValue> {
    fn from(value: DictionaryEntry) -> Self {
        let cont: Rc<dyn StackValue> = Rc::new(value.definition);
        if value.active {
            Rc::new(vec![cont])
        } else {
            cont
        }
    }
}

pub struct Dictionary {
    words: Rc<SharedBox>,
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

    pub fn words(&self) -> &Rc<SharedBox> {
        &self.words
    }

    pub fn lookup(&self, name: &String) -> Result<Option<DictionaryEntry>> {
        let words = self.words.fetch();
        let map = match words.ty() {
            StackValueType::Null => None,
            _ => Some(words.into_hashmap()?),
        };
        let key = HashMapTreeKeyRef::from(name);
        let Some(node) = HashMapTreeNode::lookup(&map, key) else {
            return Ok(None);
        };
        Ok(DictionaryEntry::try_from_value(node.value.as_ref()))
    }

    pub fn resolve_name(&self, definition: &dyn ContImpl) -> Option<Rc<String>> {
        let map = self.words.borrow();
        if let Ok(map) = map.as_hashmap() {
            for entry in map {
                let Some((cont, _)) = DictionaryEntry::cont_from_value(entry.value.as_ref()) else {
                    continue;
                };

                // NOTE: erase trait data from fat pointers
                let left = Rc::as_ptr(cont) as *const ();
                let right = definition as *const _ as *const ();
                // Compare only the address part
                if std::ptr::eq(left, right) {
                    return entry.key.stack_value.clone().into_string().ok();
                }
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
            words: &SharedBox,
            name: String,
            word: DictionaryEntry,
            _: bool,
        ) -> Result<()> {
            let old_words = words.take();
            let mut map = if old_words.is_null() {
                None
            } else {
                Some(old_words.into_hashmap()?)
            };

            let key = HashMapTreeKey::new(Rc::new(name))?;
            let value = &word.into();
            HashMapTreeNode::set(&mut map, &key, value);

            words.store_opt(map);
            Ok(())
        }
        define_word_impl(&self.words, name.into(), word.into(), allow_redefine)
    }

    pub fn undefine_word(&mut self, name: &String) -> Result<bool> {
        let old_words = self.words.take();
        let mut map = if old_words.is_null() {
            None
        } else {
            Some(old_words.into_hashmap()?)
        };

        let key = HashMapTreeKeyRef::from(name);
        let res = HashMapTreeNode::remove(&mut map, key).is_some();
        self.words.store_opt(map);
        Ok(res)
    }
}
