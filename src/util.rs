use everscale_types::prelude::*;
use num_bigint::BigInt;
use num_traits::Num;
use unicode_segmentation::UnicodeSegmentation;

use crate::error::*;

pub struct ImmediateInt {
    pub num: BigInt,
    pub denom: Option<BigInt>,
}

impl ImmediateInt {
    pub fn try_from_str(s: &str) -> Result<Option<Self>> {
        let (num, denom) = if let Some((left, right)) = s.split_once('/') {
            let Some(num) = Self::parse_single_number(left)? else {
                return Ok(None);
            };
            let Some(denom) = Self::parse_single_number(right)? else {
                return Err(Error::InvalidNumber);
            };
            (num, Some(denom))
        } else {
            let Some(num) = Self::parse_single_number(s)? else {
                return Ok(None);
            };
            (num, None)
        };
        Ok(Some(ImmediateInt { num, denom }))
    }

    fn parse_single_number(s: &str) -> Result<Option<BigInt>> {
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
        .map_err(|_| Error::InvalidNumber)?;

        if neg {
            num = -num;
        }

        Ok(Some(num))
    }
}

pub fn reverse_utf8_string_inplace(s: &mut str) {
    unsafe {
        let v = s.as_bytes_mut();

        // Reverse the bytes within each grapheme cluster.
        // This does not preserve UTF-8 validity.
        {
            // Invariant: `tail` points to data we have not modified yet, so it is always valid UTF-8.
            let mut tail = &mut v[..];
            while let Some(len) = std::str::from_utf8_unchecked(tail)
                .graphemes(true)
                .next()
                .map(str::len)
            {
                let (grapheme, new_tail) = tail.split_at_mut(len);
                grapheme.reverse();
                tail = new_tail;
            }
        }

        // Reverse all bytes. This restores multi-byte sequences to their original order.
        v.reverse();

        // The string is now valid UTF-8 again.
        debug_assert!(std::str::from_utf8(v).is_ok());
    }
}

pub trait DisplaySliceExt<'s> {
    fn display_slice_tree<'a: 's>(&'a self, limit: usize) -> DisplayCellSlice<'a, 's>;

    fn display_slice_data<'a: 's>(&'a self) -> DisplaySliceData<'a, 's>;
}

impl<'s> DisplaySliceExt<'s> for CellSlice<'s> {
    fn display_slice_tree<'a: 's>(&'a self, limit: usize) -> DisplayCellSlice<'a, 's> {
        DisplayCellSlice { slice: self, limit }
    }

    fn display_slice_data<'a: 's>(&'a self) -> DisplaySliceData<'a, 's> {
        DisplaySliceData(self)
    }
}

pub struct DisplayCellSlice<'a, 'b> {
    slice: &'a CellSlice<'b>,
    limit: usize,
}

impl std::fmt::Display for DisplayCellSlice<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut stack = vec![(0, *self.slice)];

        let mut i = 0;
        while let Some((indent, cs)) = stack.pop() {
            i += 1;
            if i > self.limit {
                return f.write_str("<cell output limit reached>\n");
            }

            writeln!(f, "{:indent$}{}", "", DisplaySliceData(&cs))?;

            for cell in cs.references().rev() {
                // SAFETY: it is safe to print pruned branches
                let cs = unsafe { cell.as_slice_unchecked() };
                stack.push((indent + 1, cs));
            }
        }

        Ok(())
    }
}

pub struct DisplaySliceData<'a, 'b>(&'a CellSlice<'b>);

impl std::fmt::Display for DisplaySliceData<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cs = *self.0;

        if cs.cell_type().is_exotic() {
            f.write_str("SPECIAL ")?;
        }

        let mut buffer: [u8; 128] = [0; 128];

        let bits = cs.remaining_bits();
        cs.load_raw(&mut buffer, bits)
            .map_err(|_| std::fmt::Error)?;
        append_tag(&mut buffer, bits);

        let mut result = hex::encode(&buffer[..(bits as usize + 7) / 8]);
        if bits % 8 <= 4 {
            result.pop();
        }
        if bits % 4 != 0 {
            result.push('_');
        }

        write!(f, "x{{{}}}", result)
    }
}

fn append_tag(data: &mut [u8; 128], bit_len: u16) {
    debug_assert!(bit_len < 1024);

    let rem = bit_len % 8;
    let last_byte = (bit_len / 8) as usize;
    if rem > 0 {
        let last_byte = &mut data[last_byte];

        let tag_mask: u8 = 1 << (7 - rem);
        let data_mask = !(tag_mask - 1);

        *last_byte = (*last_byte & data_mask) | tag_mask;
    }
}
