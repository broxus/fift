use anyhow::Result;
use tycho_vm::SafeRc;

use super::cont::{RcFiftCont, FiftCont, ContextTailWordFunc, ContextWordFunc, StackWordFunc};
use super::stack::{
    DynFiftValue, HashMapTreeKey, HashMapTreeKeyRef, HashMapTreeNode, SharedBox, StackValue,
    StackValueType,
};
use super::{DynFiftCont, IntoDynFiftCont};

pub struct Dictionaries {
    pub current: Dictionary,
    pub original: Dictionary,
    pub context: Dictionary,
}

impl Default for Dictionaries {
    fn default() -> Self {
        let current = Dictionary::default();
        Self {
            original: current.clone(),
            context: current.clone(),
            current,
        }
    }
}

impl Dictionaries {
    pub fn lookup(&self, word: &String, allow_space: bool) -> Result<Option<DictionaryEntry>> {
        if allow_space {
            let mut entry = self.lookup(word, false)?;

            if entry.is_none() {
                entry = self.lookup(&format!("{word} "), false)?;
            }

            return Ok(entry);
        }

        let mut entry = self.context.lookup(word)?;

        if entry.is_none() && self.current != self.context {
            entry = self.current.lookup(word)?;
        }

        if entry.is_none() && self.original != self.context && self.original != self.current {
            entry = self.original.lookup(word)?;
        }

        Ok(entry)
    }
}

#[derive(Default, Clone, Eq, PartialEq)]
pub struct Dictionary {
    words: SafeRc<SharedBox>,
}

impl Dictionary {
    pub fn set_words_box(&mut self, words: SafeRc<SharedBox>) {
        self.words = words;
    }

    pub fn get_words_box(&self) -> &SafeRc<SharedBox> {
        &self.words
    }

    pub fn clone_words_map(&self) -> Result<Option<SafeRc<HashMapTreeNode>>> {
        let words = self.words.fetch();
        Ok(match words.ty() {
            StackValueType::Null => None,
            _ => Some(words.into_hashmap()?),
        })
    }

    pub fn use_words_map(&mut self) -> Result<WordsRefMut<'_>> {
        let words = self.words.take();
        let map = if words.is_null() {
            None
        } else {
            Some(words.into_hashmap()?)
        };
        Ok(WordsRefMut {
            words_box: &mut self.words,
            map,
        })
    }

    pub fn lookup(&self, name: &String) -> Result<Option<DictionaryEntry>> {
        let map = self.clone_words_map()?;
        let key = HashMapTreeKeyRef::from(name);
        let Some(node) = HashMapTreeNode::lookup(&map, key) else {
            return Ok(None);
        };
        Ok(DictionaryEntry::try_from_value(node.value.as_ref()))
    }

    pub fn resolve_name(&self, definition: &dyn FiftCont) -> Option<SafeRc<String>> {
        let map = self.words.borrow();
        if let Ok(map) = map.as_hashmap() {
            for entry in map {
                let Some((cont, _)) = DictionaryEntry::cont_from_value(entry.value.as_ref()) else {
                    continue;
                };

                // NOTE: erase trait data from fat pointers
                let left = SafeRc::as_ptr(cont) as *const ();
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
        self.define_word(name, DictionaryEntry {
            definition: SafeRc::new_dyn_fift_cont(f),
            active: false,
        })
    }

    pub fn define_context_tail_word<T: Into<String>>(
        &mut self,
        name: T,
        f: ContextTailWordFunc,
    ) -> Result<()> {
        self.define_word(name, DictionaryEntry {
            definition: SafeRc::new_dyn_fift_cont(f),
            active: false,
        })
    }

    pub fn define_active_word<T: Into<String>>(
        &mut self,
        name: T,
        f: ContextWordFunc,
    ) -> Result<()> {
        self.define_word(name, DictionaryEntry {
            definition: SafeRc::new_dyn_fift_cont(f),
            active: true,
        })
    }

    pub fn define_stack_word<T: Into<String>>(&mut self, name: T, f: StackWordFunc) -> Result<()> {
        self.define_word(name, DictionaryEntry {
            definition: SafeRc::new_dyn_fift_cont(f),
            active: false,
        })
    }

    pub fn define_word<T, E>(&mut self, name: T, word: E) -> Result<()>
    where
        T: Into<String>,
        E: Into<DictionaryEntry>,
    {
        fn define_word_impl(d: &mut Dictionary, name: String, word: DictionaryEntry) -> Result<()> {
            let mut map = d.use_words_map()?;

            let key = HashMapTreeKey::from(name);
            let value = &word.into();
            HashMapTreeNode::set(&mut map, &key, value);
            Ok(())
        }
        define_word_impl(self, name.into(), word.into())
    }

    pub fn undefine_word(&mut self, name: &String) -> Result<bool> {
        let mut map = self.use_words_map()?;

        let key = HashMapTreeKeyRef::from(name);
        Ok(HashMapTreeNode::remove(&mut map, key).is_some())
    }
}

pub struct DictionaryEntry {
    pub definition: RcFiftCont,
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

    fn cont_from_value(value: &dyn StackValue) -> Option<(&RcFiftCont, bool)> {
        if let Ok(cont) = value.as_cont() {
            return Some((cont, false));
        } else if let Ok(tuple) = value.as_tuple()
            && tuple.len() == 1
            && let Ok(cont) = tuple.first()?.as_cont()
        {
            return Some((cont, true));
        }
        None
    }
}

impl From<RcFiftCont> for DictionaryEntry {
    fn from(value: RcFiftCont) -> Self {
        Self {
            definition: value,
            active: false,
        }
    }
}

impl<T: FiftCont + 'static> From<SafeRc<T>> for DictionaryEntry {
    fn from(value: SafeRc<T>) -> Self {
        Self {
            definition: value.into_dyn_fift_cont(),
            active: false,
        }
    }
}

impl From<DictionaryEntry> for SafeRc<dyn StackValue> {
    fn from(value: DictionaryEntry) -> Self {
        let cont = SafeRc::new_dyn_fift_value(value.definition);
        if value.active {
            SafeRc::new_dyn_fift_value(vec![cont])
        } else {
            cont
        }
    }
}

pub struct WordsRefMut<'a> {
    words_box: &'a mut SafeRc<SharedBox>,
    map: Option<SafeRc<HashMapTreeNode>>,
}

impl std::ops::Deref for WordsRefMut<'_> {
    type Target = Option<SafeRc<HashMapTreeNode>>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl std::ops::DerefMut for WordsRefMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl Drop for WordsRefMut<'_> {
    fn drop(&mut self) {
        self.words_box.store_opt(self.map.take());
    }
}
