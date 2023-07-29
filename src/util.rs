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
