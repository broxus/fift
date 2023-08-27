use anyhow::{Context, Result};

use super::env::SourceBlock;
use crate::error::UnexpectedEof;

#[derive(Default)]
pub struct Lexer {
    blocks: Vec<SourceBlockState>,
}

impl Lexer {
    pub fn push_source_block(&mut self, block: SourceBlock) {
        self.blocks.push(SourceBlockState::from(block));
    }

    pub fn pop_source_block(&mut self) -> bool {
        self.blocks.pop().is_some()
    }

    pub fn reset_until_base(&mut self) {
        self.blocks.truncate(1);
    }

    pub fn get_position(&self) -> Option<LexerPosition<'_>> {
        let offset = self.blocks.len();
        let input = self.blocks.last()?;
        Some(LexerPosition {
            offset,
            source_block_name: input.block.name(),
            line: &input.line,
            word_start: input.prev_word_start,
            word_end: input.prev_word_end,
            line_number: input.line_number,
        })
    }

    pub fn depth(&self) -> i32 {
        (self.blocks.len() as i32) - 1
    }

    pub fn scan_word(&mut self) -> Result<Option<&str>> {
        let Some(input) = self.blocks.last_mut() else {
            return Ok(None);
        };
        input.scan_word()
    }

    pub fn scan_until_space_or_eof(&mut self) -> Result<&str> {
        if let Some(input) = self.blocks.last_mut() {
            if let Some(word) = input.scan_word()? {
                return Ok(word);
            }
        }
        Ok("")
    }

    pub fn scan_until_delimiter(&mut self, delimiter: char) -> Result<&str> {
        self.use_last_block()?.scan_until(delimiter)
    }

    pub fn scan_classify(&mut self, delims: &str, space_class: u8) -> Result<&str> {
        let Some(input) = self.blocks.last_mut() else {
            return Ok("");
        };
        let classifier = AsciiCharClassifier::with_delims(delims, space_class)?;
        input.scan_classify(&classifier)
    }

    pub fn rewind(&mut self, last_word_len: usize) {
        if let Some(input) = self.blocks.last_mut() {
            input.rewind(last_word_len)
        }
    }

    pub fn scan_skip_whitespace(&mut self) -> Result<bool> {
        if let Some(input) = self.blocks.last_mut() {
            input.scan_skip_whitespace()
        } else {
            Ok(false)
        }
    }

    pub fn skip_line_whitespace(&mut self) {
        if let Some(input) = self.blocks.last_mut() {
            input.skip_line_whitespace();
        }
    }

    pub fn skip_symbol(&mut self) {
        if let Some(input) = self.blocks.last_mut() {
            input.skip_symbol();
        }
    }

    fn use_last_block(&mut self) -> Result<&mut SourceBlockState> {
        self.blocks.last_mut().ok_or_else(|| UnexpectedEof.into())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LexerPosition<'a> {
    pub offset: usize,
    pub source_block_name: &'a str,
    pub line: &'a str,
    pub word_start: usize,
    pub word_end: usize,
    pub line_number: usize,
}

pub trait Delimiter {
    fn delim(&mut self, c: char) -> bool;
    fn is_eof(&self) -> bool;
}

impl<T: FnMut(char) -> bool> Delimiter for T {
    fn delim(&mut self, c: char) -> bool {
        (self)(c)
    }

    fn is_eof(&self) -> bool {
        false
    }
}

impl Delimiter for char {
    #[inline]
    fn delim(&mut self, c: char) -> bool {
        *self == c
    }

    fn is_eof(&self) -> bool {
        *self as u32 == 0
    }
}

struct SourceBlockState {
    block: SourceBlock,
    line: String,
    require_next_line: bool,
    line_offset: usize,
    prev_word_start: usize,
    prev_word_end: usize,
    line_number: usize,
}

impl From<SourceBlock> for SourceBlockState {
    fn from(block: SourceBlock) -> Self {
        Self {
            block,
            line: Default::default(),
            require_next_line: true,
            line_offset: 0,
            prev_word_start: 0,
            prev_word_end: 0,
            line_number: 0,
        }
    }
}

impl SourceBlockState {
    fn scan_word(&mut self) -> Result<Option<&str>> {
        loop {
            if !self.scan_skip_whitespace()? {
                return Ok(None);
            }

            let start = self.line_offset;
            self.prev_word_start = start;

            self.skip_until(char::is_whitespace);
            let end = self.line_offset;
            self.prev_word_end = end;

            self.skip_line_whitespace();

            if start != end {
                return Ok(Some(&self.line[start..end]));
            }
        }
    }

    fn scan_until(&mut self, c: char) -> Result<&str> {
        if self.require_next_line {
            self.read_line()?;
        }

        let start = self.line_offset;
        self.prev_word_start = start;

        let mut found = false;
        self.skip_until(|x| {
            found |= x == c;
            found
        });

        let end = self.line_offset;
        self.prev_word_end = self.line_offset;

        anyhow::ensure!(found || c as u32 == 0, "End delimiter `{c}` not found");

        if found {
            self.skip_symbol();
        } else {
            self.require_next_line = true;
        }

        Ok(&self.line[start..end])
    }

    fn scan_classify(&mut self, classifier: &AsciiCharClassifier) -> Result<&str> {
        self.scan_skip_whitespace()?;

        let start = self.line_offset;
        self.prev_word_start = start;

        let mut skip = false;
        let mut empty = true;
        self.skip_until(|c| {
            if c == '\n' || c == '\r' {
                return true;
            }

            let class = classifier.classify(c);
            if class & 0b01 != 0 && !empty {
                return true;
            } else if class & 0b10 != 0 {
                skip = true;
                return true;
            }

            empty = false;
            false
        });

        if skip {
            self.skip_symbol();
        }

        self.prev_word_end = self.line_offset;

        Ok(&self.line[start..self.line_offset])
    }

    fn rewind(&mut self, last_word_len: usize) {
        self.line_offset = self.prev_word_start + last_word_len;
        self.prev_word_end = self.line_offset;
    }

    fn scan_skip_whitespace(&mut self) -> Result<bool> {
        loop {
            self.skip_line_whitespace();

            if self.line_offset < self.line.len() {
                return Ok(true);
            }

            if (self.line.is_empty() || self.line_offset >= self.line.len()) && !self.read_line()? {
                return Ok(false);
            }
        }
    }

    fn skip_line_whitespace(&mut self) {
        self.skip_while(char::is_whitespace)
    }

    fn skip_until<P: Delimiter>(&mut self, mut p: P) {
        self.skip_while(|c| !p.delim(c));
    }

    fn skip_symbol(&mut self) {
        let mut first = true;
        self.skip_while(|_| std::mem::take(&mut first))
    }

    fn skip_while<P: Delimiter>(&mut self, mut p: P) {
        let prev_offset = self.line_offset;
        for (offset, c) in self.line[self.line_offset..].char_indices() {
            if !p.delim(c) {
                self.line_offset = prev_offset + offset;
                return;
            }
        }
        self.line_offset = self.line.len();
    }

    fn read_line(&mut self) -> Result<bool> {
        const SKIP_PREFIX: &str = "#!";

        self.require_next_line = false;
        self.prev_word_start = 0;
        self.prev_word_end = 0;
        self.line_offset = 0;
        self.line_number += 1;
        self.line.clear();
        let not_eof = self.block.buffer_mut().read_line(&mut self.line)? > 0;
        if not_eof && self.line_number == 1 && self.line.starts_with(SKIP_PREFIX) {
            self.read_line()
        } else {
            Ok(not_eof)
        }
    }
}

struct AsciiCharClassifier {
    /// A native representation of `[u2; 256]`
    data: [u8; 64],
}

impl AsciiCharClassifier {
    fn with_delims(delims: &str, space_class: u8) -> Result<Self> {
        anyhow::ensure!(
            delims.is_ascii(),
            "Non-ascii symbols are not supported by character classifier"
        );

        let mut data = [0u8; 64];
        let mut set_char_class = |c: u8, mut class: u8| {
            // Ensure that class is in range 0..=3
            class &= 0b11;

            let offset = (c & 0b11) * 2;

            // Each byte stores classes (0..=3) for 4 characters.
            // 0: 00 00 00 11
            // 1: 00 00 11 00
            // 2: 00 11 00 00
            // 3: 11 00 00 00
            let mask = 0b11 << offset;
            class <<= offset;

            // Find a byte for the character
            let p = &mut data[(c >> 2) as usize];
            // Set character class whithin this byte
            *p = (*p & !mask) | class;
        };

        set_char_class(b' ', space_class);
        set_char_class(b'\t', space_class);

        let mut class = 0b11u8;
        for &c in delims.as_bytes() {
            if c == b' ' {
                class = class.checked_sub(1).context("Too many classes")?;
            } else {
                set_char_class(c, class);
            }
        }

        Ok(Self { data })
    }

    fn classify(&self, c: char) -> u8 {
        if c.is_ascii() {
            let c = c as u8;
            let offset = (c & 0b11) * 2;
            (self.data[(c >> 2) as usize] >> offset) & 0b11
        } else {
            0
        }
    }
}
