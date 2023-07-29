use std::io::BufRead;

use num_bigint::BigInt;
use num_traits::Num;

use crate::error::*;

pub struct Lexer<'a> {
    input: &'a mut dyn BufRead,
    line: String,
    line_offset: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a mut dyn BufRead) -> Self {
        Self {
            input,
            line: Default::default(),
            line_offset: 0,
        }
    }

    pub fn scan_word(&mut self) -> FiftResult<Option<Token<'_>>> {
        loop {
            if (self.line.is_empty() || self.line_offset >= self.line.len()) && !self.read_line()? {
                return Ok(None);
            }

            self.skip_whitespace();
            let start = self.line_offset;
            self.skip_until(char::is_whitespace);
            let end = self.line_offset;

            if start == end {
                continue;
            }

            return Ok(Some(Token {
                data: &self.line[start..end],
            }));
        }
    }

    pub fn scan_word_until<P: Delimiter>(&mut self, mut p: P) -> FiftResult<Token<'_>> {
        if (self.line.is_empty() || self.line_offset >= self.line.len()) && !self.read_line()? {
            return Err(FiftError::UnexpectedEof);
        }

        let start = self.line_offset;

        let mut found = false;
        self.skip_until(|c| {
            found |= p.delim(c);
            found
        });

        let end = self.line_offset;

        if found && end >= start {
            self.skip_symbol();
            Ok(Token {
                data: &self.line[start..end],
            })
        } else {
            Err(FiftError::UnexpectedEof)
        }
    }

    pub fn rewind(&mut self, offset: usize) {
        self.line_offset -= offset;
    }

    pub fn skip_whitespace(&mut self) {
        self.skip_while(char::is_whitespace)
    }

    pub fn skip_until<P: Delimiter>(&mut self, mut p: P) {
        self.skip_while(|c| !p.delim(c));
    }

    pub fn skip_symbol(&mut self) {
        let mut first = true;
        self.skip_while(|_| std::mem::take(&mut first))
    }

    pub fn skip_while<P: Delimiter>(&mut self, mut p: P) {
        let prev_offset = self.line_offset;
        for (offset, c) in self.line[self.line_offset..].char_indices() {
            if !p.delim(c) {
                self.line_offset = prev_offset + offset;
                return;
            }
        }
        self.line_offset = self.line.len();
    }

    fn read_line(&mut self) -> FiftResult<bool> {
        let n = self.input.read_line(&mut self.line)?;
        Ok(n > 0)
    }
}

pub struct Token<'a> {
    pub data: &'a str,
}

impl Token<'_> {
    pub fn subtokens(&self) -> Subtokens {
        Subtokens(self.data)
    }

    pub fn delta(&self, subtoken: &str) -> usize {
        self.data.len() - subtoken.len()
    }

    pub fn parse_number(&self) -> FiftResult<Option<ImmediateInt>> {
        let (num, denom) = if let Some((left, right)) = self.data.split_once('/') {
            let Some(num) = Self::parse_single_number(left)? else {
                return Ok(None);
            };
            let Some(denom) = Self::parse_single_number(right)? else {
                return Err(FiftError::InvalidNumber);
            };
            (num, Some(denom))
        } else {
            let Some(num) = Self::parse_single_number(self.data)? else {
                return Ok(None);
            };
            (num, None)
        };
        Ok(Some(ImmediateInt { num, denom }))
    }

    fn parse_single_number(s: &str) -> FiftResult<Option<BigInt>> {
        let (neg, s) = match s.strip_prefix('-') {
            Some(s) => (true, s),
            None => (false, s),
        };

        let mut num = if let Some(s) = s.strip_prefix("0x") {
            BigInt::from_str_radix(s, 16)
        } else if let Some(s) = s.strip_prefix("0b") {
            BigInt::from_str_radix(s, 2)
        } else {
            if !s.chars().all(|c| c.is_ascii_digit()) {
                return Ok(None);
            }
            BigInt::from_str_radix(s, 10)
        }
        .map_err(|_| FiftError::InvalidNumber)?;

        if neg {
            num = -num;
        }

        Ok(Some(num))
    }
}

pub struct Subtokens<'a>(&'a str);

impl<'a> Iterator for Subtokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let (i, _) = self.0.char_indices().next_back()?;
        let res = self.0;
        self.0 = &res[..i];
        Some(res)
    }
}

pub trait Delimiter {
    fn delim(&mut self, c: char) -> bool;
}

impl<T: FnMut(char) -> bool> Delimiter for T {
    fn delim(&mut self, c: char) -> bool {
        (self)(c)
    }
}

impl Delimiter for char {
    #[inline]
    fn delim(&mut self, c: char) -> bool {
        *self == c
    }
}

pub struct ImmediateInt {
    pub num: BigInt,
    pub denom: Option<BigInt>,
}
