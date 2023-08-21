use std::rc::Rc;
use std::str::FromStr;

use anyhow::Result;
use everscale_types::models::StdAddr;
use everscale_types::prelude::HashBytes;
use num_bigint::{BigInt, Sign};
use num_traits::Num;
use sha2::Digest;

use crate::core::*;
use crate::error::UnexpectedEof;
use crate::util::*;

pub struct StringUtils;

#[fift_module]
impl StringUtils {
    #[cmd(name = "\"", active, without_space)]
    fn interpret_quote_str(ctx: &mut Context) -> Result<()> {
        let word = ctx.input.scan_until('"')?;
        ctx.stack.push(word.to_owned())?;
        ctx.stack.push_argcount(1)
    }

    #[cmd(name = "char", active)]
    fn interpret_char(ctx: &mut Context) -> Result<()> {
        let token = ctx.input.scan_word()?.ok_or(UnexpectedEof)?;
        let mut chars = token.chars();
        let char = chars.next().ok_or(UnexpectedEof)?;
        anyhow::ensure!(chars.next().is_none(), "Expected exactly one character");
        ctx.stack.push_int(char as u32)?;
        ctx.stack.push_argcount(1)
    }

    #[cmd(name = "(char)", stack)]
    fn interpret_char_internal(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let mut chars = string.chars();
        let char = chars.next().ok_or(UnexpectedEof)?;
        anyhow::ensure!(chars.next().is_none(), "Expected exactly one character");
        stack.push_int(char as u32)
    }

    #[cmd(name = "emit")]
    fn interpret_emit(ctx: &mut Context) -> Result<()> {
        let c = ctx.stack.pop_smallint_char()?;
        write!(ctx.stdout, "{c}")?;
        Ok(())
    }

    #[cmd(name = "space", args(c = ' '))]
    #[cmd(name = "cr", args(c = '\n'))]
    fn interpret_emit_const(ctx: &mut Context, c: char) -> Result<()> {
        write!(ctx.stdout, "{c}")?;
        Ok(())
    }

    #[cmd(name = "type")]
    fn interpret_type(ctx: &mut Context) -> Result<()> {
        let string = ctx.stack.pop_string()?;
        write!(ctx.stdout, "{string}")?;
        Ok(())
    }

    #[cmd(name = "chr", stack)]
    fn interpret_chr(stack: &mut Stack) -> Result<()> {
        let c = stack.pop_smallint_char()?;
        stack.push(c.to_string())
    }

    #[cmd(name = "hold", stack)]
    fn interpret_hold(stack: &mut Stack) -> Result<()> {
        let c = stack.pop_smallint_char()?;
        let mut string = stack.pop_string()?;
        Rc::make_mut(&mut string).push(c);
        stack.push_raw(string)
    }

    #[cmd(name = "(number)", stack)]
    fn interpret_parse_number(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let mut res = 0;
        if let Ok(Some(int)) = ImmediateInt::try_from_str(&string) {
            res += 1;
            stack.push(int.num)?;
            if let Some(denom) = int.denom {
                res += 1;
                stack.push(denom)?;
            }
        }
        stack.push_int(res)
    }

    #[cmd(name = "(hex-number)", stack)]
    fn interpret_parse_hex_number(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let (neg, s) = match string.strip_prefix('-') {
            Some(s) => (true, s),
            None => (false, string.as_str()),
        };

        let mut res = 0;
        if let Ok(mut num) = BigInt::from_str_radix(s, 16) {
            res += 1;
            if neg {
                num = -num;
            }
            stack.push(num)?;
        }
        stack.push_int(res)
    }

    #[cmd(name = "$|", stack)]
    #[cmd(name = "$Split", stack)]
    fn interpret_str_split(stack: &mut Stack) -> Result<()> {
        let at = stack.pop_smallint_range(0, i32::MAX as _)? as usize;
        let mut head = stack.pop_string()?;

        anyhow::ensure!(at <= head.len(), "Index out of range");
        anyhow::ensure!(head.is_char_boundary(at), "Index is not the char boundary");

        let tail = Rc::new(head[at..].to_owned());
        Rc::make_mut(&mut head).truncate(at);

        stack.push_raw(head)?;
        stack.push_raw(tail)
    }

    #[cmd(name = "$+", stack)]
    fn interpret_str_concat(stack: &mut Stack) -> Result<()> {
        let tail = stack.pop_string()?;
        let mut head = stack.pop_string()?;
        Rc::make_mut(&mut head).push_str(&tail);
        stack.push_raw(head)
    }

    #[cmd(name = "$=", stack)]
    fn interpret_str_equal(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop_string()?;
        let rhs = stack.pop_string()?;
        stack.push_bool(lhs == rhs)
    }

    #[cmd(name = "$cmp", stack)]
    fn interpret_str_cmp(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop_string()?;
        let rhs = stack.pop_string()?;
        stack.push_int(lhs.cmp(&rhs) as i8)
    }

    #[cmd(name = "$reverse", stack)]
    fn interpret_str_reverse(stack: &mut Stack) -> Result<()> {
        let mut string = stack.pop_string()?;
        reverse_utf8_string_inplace(Rc::make_mut(&mut string).as_mut_str());
        stack.push_raw(string)
    }

    #[cmd(name = "$pos", stack)]
    #[cmd(name = "$Pos", stack)]
    fn interpret_str_pos(stack: &mut Stack) -> Result<()> {
        let substring = stack.pop_string()?;
        let string = stack.pop_string()?;
        stack.push(match string.find(substring.as_str()) {
            Some(idx) => BigInt::from(idx),
            None => BigInt::from(-1),
        })
    }

    // $at (S n -- S')
    #[cmd(name = "$at", stack)]
    fn interpret_str_at(stack: &mut Stack) -> Result<()> {
        let index = stack.pop_usize()?;
        let string = stack.pop_string()?;

        match string.chars().nth(index) {
            Some(s) => stack.push(s.to_string()),
            None => anyhow::bail!("index must be >= 0 and <= {}", string.len()),
        }
    }

    // $mul (S n -- S*n)
    #[cmd(name = "$mul", stack)]
    fn interpret_str_mul(stack: &mut Stack) -> Result<()> {
        let factor = stack.pop_usize()?;
        let string = stack.pop_string()?;

        stack.push(string.repeat(factor))
    }

    // $sybs (S -- t[S'0, S'1, S'2, ..., S'n])
    #[cmd(name = "$sybs", stack)]
    fn interpret_str_sybs(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let symbols = string
            .chars()
            .map(|c| Rc::new(c.to_string()) as Rc<dyn StackValue>)
            .collect::<Vec<_>>();

        stack.push(symbols)
    }

    // $sub (S x y -- S')
    #[cmd(name = "$sub", stack)]
    fn interpret_str_sub(stack: &mut Stack) -> Result<()> {
        let y = stack.pop_usize()?;
        let x = stack.pop_usize()?;
        let string = stack.pop_string()?;

        let len = string.len();
        anyhow::ensure!(x <= y, "x must be <= y, but x is {x}");
        anyhow::ensure!(
            x <= len && y <= len,
            "x, y must be <= {len} (string length)"
        );

        stack.push(string[x..y].to_string())
    }

    // $sep (S S1 -- t(...))
    #[cmd(name = "$sep", stack)]
    fn interpret_str_split_by_str(stack: &mut Stack) -> Result<()> {
        let sep = stack.pop_string()?;
        let string = stack.pop_string()?;

        let substrings = string
            .split(sep.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| Rc::new(s.to_string()) as Rc<dyn StackValue>)
            .collect::<Vec<_>>();

        stack.push(substrings)
    }

    #[cmd(name = "$rep", stack, args(pop_n = false))] // $rep  (S S1 S2   -- S')
    #[cmd(name = "$repn", stack, args(pop_n = true))] // $repn (S S1 S2 n -- S')
    fn interpret_str_replace(stack: &mut Stack, pop_n: bool) -> Result<()> {
        let n = if pop_n { stack.pop_usize()? } else { 1 };

        let s2 = stack.pop_string()?;
        let s1 = stack.pop_string()?;
        let string = stack.pop_string()?;

        stack.push(string.replacen(s1.as_str(), s2.as_str(), n))
    }

    #[cmd(name = "$repm", stack)] // $repm (S S1 S2 -- S')
    fn interpret_str_replace_max(stack: &mut Stack) -> Result<()> {
        let s2 = stack.pop_string()?;
        let s1 = stack.pop_string()?;
        let string = stack.pop_string()?;

        stack.push(string.replace(s1.as_str(), s2.as_str()))
    }

    #[cmd(name = "(-trailing)", stack, args(arg = None))]
    #[cmd(name = "-trailing", stack, args(arg = Some(' ')))]
    #[cmd(name = "-trailing0", stack, args(arg = Some('0')))]
    fn interpret_str_remove_trailing_int(stack: &mut Stack, arg: Option<char>) -> Result<()> {
        let arg = match arg {
            Some(arg) => arg,
            None => stack.pop_smallint_char()?,
        };
        let mut string = stack.pop_string()?;
        {
            let string = Rc::make_mut(&mut string);
            string.truncate(string.trim_end_matches(arg).len());
        }
        stack.push_raw(string)
    }

    #[cmd(name = "$len", stack)]
    fn interpret_str_len(stack: &mut Stack) -> Result<()> {
        let len = stack.pop()?.as_string()?.len();
        stack.push_int(len)
    }

    #[cmd(name = "Blen", stack)]
    fn interpret_bytes_len(stack: &mut Stack) -> Result<()> {
        let len = stack.pop()?.as_bytes()?.len();
        stack.push_int(len)
    }

    #[cmd(name = "$Len", stack)]
    fn interpret_utf8_str_len(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let len = string.chars().count();
        stack.push_int(len)
    }

    #[cmd(name = "B>X", stack, args(upper = true))]
    #[cmd(name = "B>x", stack, args(upper = false))]
    fn interpret_bytes_to_hex(stack: &mut Stack, upper: bool) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let string = if upper {
            hex::encode_upper(&*bytes)
        } else {
            hex::encode(&*bytes)
        };
        stack.push(string)
    }

    #[cmd(name = "x>B", stack, args(partial = false))]
    #[cmd(name = "x>B?", stack, args(partial = true))]
    fn interpret_hex_to_bytes(stack: &mut Stack, partial: bool) -> Result<()> {
        let string = stack.pop_string()?;
        let mut string = string.as_str();
        if partial {
            let len = string
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(string.len())
                & (usize::MAX - 1);
            string = &string[..len];
        }

        let i = string.len();
        let bytes = hex::decode(string)?;

        stack.push(bytes)?;
        if partial {
            stack.push_int(i)?;
        }
        Ok(())
    }

    #[cmd(name = "B|", stack)]
    fn interpret_bytes_split(stack: &mut Stack) -> Result<()> {
        let at = stack.pop_smallint_range(0, i32::MAX as _)? as usize;
        let mut head = stack.pop_bytes()?;
        anyhow::ensure!(at <= head.len(), "Index out of range");
        let tail = Rc::new(head[at..].to_owned());
        Rc::make_mut(&mut head).truncate(at);

        stack.push_raw(head)?;
        stack.push_raw(tail)
    }

    #[cmd(name = "B+", stack)]
    fn interpret_bytes_concat(stack: &mut Stack) -> Result<()> {
        let tail = stack.pop_bytes()?;
        let mut head = stack.pop_bytes()?;
        Rc::make_mut(&mut head).extend_from_slice(&tail);
        stack.push_raw(head)
    }

    #[cmd(name = "B=", stack)]
    fn interpret_bytes_equal(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop_bytes()?;
        let rhs = stack.pop_bytes()?;
        stack.push_bool(lhs == rhs)
    }

    #[cmd(name = "Bcmp", stack)]
    fn interpret_bytes_cmp(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop_bytes()?;
        let rhs = stack.pop_bytes()?;
        stack.push_int(lhs.cmp(&rhs) as i8)
    }

    #[cmd(name = "u>B", stack, args(sgn = false, le = false))]
    #[cmd(name = "i>B", stack, args(sgn = true, le = false))]
    #[cmd(name = "Lu>B", stack, args(sgn = false, le = true))]
    #[cmd(name = "Li>B", stack, args(sgn = true, le = true))]
    fn interpret_int_to_bytes(stack: &mut Stack, sgn: bool, le: bool) -> Result<()> {
        let bits = stack.pop_smallint_range(1, if sgn { 264 } else { 256 })?;
        let int = stack.pop_int()?;
        anyhow::ensure!(bits % 8 == 0, "Can store only an integer number of bytes");
        anyhow::ensure!(
            int.bits() <= bits as _,
            "Integer does not fit into the buffer"
        );

        let byte_len = (bits / 8) as usize;
        let (prefix, mut bytes) = if sgn {
            let bytes = int.to_signed_bytes_le();
            (
                bytes
                    .last()
                    .map(|first| (first >> 7) * 255)
                    .unwrap_or_default(),
                bytes,
            )
        } else {
            (0, int.to_bytes_le().1)
        };
        bytes.resize(byte_len, prefix);
        if !le {
            bytes.reverse();
        }
        stack.push(bytes)
    }

    #[cmd(name = "B>u@", stack, args(sgn = false, adv = false, le = false))]
    #[cmd(name = "B>i@", stack, args(sgn = true, adv = false, le = false))]
    #[cmd(name = "B>u@+", stack, args(sgn = false, adv = true, le = false))]
    #[cmd(name = "B>i@+", stack, args(sgn = true, adv = true, le = false))]
    #[cmd(name = "B>Lu@", stack, args(sgn = false, adv = false, le = true))]
    #[cmd(name = "B>Li@", stack, args(sgn = true, adv = false, le = true))]
    #[cmd(name = "B>Lu@+", stack, args(sgn = false, adv = true, le = true))]
    #[cmd(name = "B>Li@+", stack, args(sgn = true, adv = true, le = true))]
    fn interpret_bytes_fetch_int(stack: &mut Stack, sgn: bool, adv: bool, le: bool) -> Result<()> {
        let bits = stack.pop_smallint_range(0, 256 + sgn as u32)?;
        let mut bytes = stack.pop_bytes()?;
        anyhow::ensure!(bits % 8 == 0, "Can load only an integer number of bytes");

        let byte_len = (bits / 8) as usize;
        anyhow::ensure!(bytes.len() >= byte_len, "Not enough bytes in the source");

        let int = {
            let data = &bytes.as_ref()[..byte_len];
            match (sgn, le) {
                (false, false) => BigInt::from_bytes_be(Sign::Plus, data),
                (false, true) => BigInt::from_bytes_le(Sign::Plus, data),
                (true, false) => BigInt::from_signed_bytes_be(data),
                (true, true) => BigInt::from_signed_bytes_le(data),
            }
        };

        if adv {
            match Rc::get_mut(&mut bytes) {
                Some(inner) => {
                    inner.copy_within(byte_len.., 0);
                    inner.truncate(inner.len() - byte_len);
                    stack.push_raw(bytes)?;
                }
                None => stack.push(bytes.as_ref()[byte_len..].to_owned())?,
            }
        }

        stack.push(int)
    }

    #[cmd(name = "$>B", stack)]
    fn interpret_string_to_bytes(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        stack.push(string.as_ref().as_bytes().to_vec())
    }

    #[cmd(name = "B>$", stack)]
    fn interpret_bytes_to_string(stack: &mut Stack) -> Result<()> {
        let bytes = stack.pop_bytes_owned()?;
        let string = String::from_utf8(bytes)?;
        stack.push(string)
    }

    #[cmd(name = "Bhash", stack, args(as_uint = true))]
    #[cmd(name = "Bhashu", stack, args(as_uint = true))]
    #[cmd(name = "BhashB", stack, args(as_uint = false))]
    fn interpret_bytes_hash(stack: &mut Stack, as_uint: bool) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let hash = sha2::Sha256::digest(&*bytes);
        if as_uint {
            stack.push(BigInt::from_bytes_be(Sign::Plus, &hash))
        } else {
            stack.push(hash.to_vec())
        }
    }

    #[cmd(name = "B>base64", stack, args(url = false))]
    #[cmd(name = "B>base64url", stack, args(url = true))]
    fn interpret_bytes_to_base64(stack: &mut Stack, url: bool) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        stack.push(if url {
            encode_base64_url(&*bytes)
        } else {
            encode_base64(&*bytes)
        })
    }

    #[cmd(name = "base64>B", stack, args(url = false))]
    #[cmd(name = "base64url>B", stack, args(url = true))]
    fn interpret_base64_to_bytes(stack: &mut Stack, url: bool) -> Result<()> {
        let string = stack.pop_string()?;
        let bytes = if url {
            decode_base64_url(&*string)?
        } else {
            decode_base64(&*string)?
        };
        stack.push(bytes)
    }

    #[cmd(name = "smca>$", stack)]
    fn interpret_pack_std_smc_addr(stack: &mut Stack) -> Result<()> {
        let mode = stack.pop_smallint_range(0, 7)? as u8;
        let int = stack.pop_int()?;
        anyhow::ensure!(int.sign() != Sign::Minus, "Expected non-negative integer");
        anyhow::ensure!(int.bits() <= 256, "Integer does not fit into the buffer");

        let workchain = stack.pop_smallint_signed_range(-0x80, 0x7f)? as i8;
        let testnet = mode & 2 != 0;
        let bounceable = mode & 1 == 0;
        let url_safe = mode & 4 != 0;

        let mut bytes = int.to_bytes_be().1;
        bytes.resize(32, 0);
        bytes.reverse();

        let mut buffer = [0u8; 36];
        buffer[0] = 0x51 - (bounceable as u8) * 0x40 + (testnet as u8) * 0x80;
        buffer[1] = workchain as u8;
        buffer[2..34].copy_from_slice(&bytes);

        let crc = CRC_16.checksum(&buffer[..34]);
        buffer[34] = (crc >> 8) as u8;
        buffer[35] = crc as u8;

        stack.push(if url_safe {
            encode_base64_url(buffer)
        } else {
            encode_base64(buffer)
        })
    }

    #[cmd(name = "$>smca", stack)]
    fn interpret_unpack_std_smc_addr(stack: &mut Stack) -> Result<()> {
        struct AddrFlags {
            testnet: bool,
            bounceable: bool,
        }

        fn unpack_base64_addr(s: &str) -> Result<(AddrFlags, StdAddr)> {
            anyhow::ensure!(s.len() == 48, "Invalid address string length");

            let buffer = match decode_base64(s) {
                Ok(buffer) => buffer,
                Err(e) => match decode_base64_url(s) {
                    Ok(buffer) => buffer,
                    Err(_) => return Err(e.into()),
                },
            };
            anyhow::ensure!(buffer.len() == 36, "Invalid decoder buffer length");

            let crc = CRC_16.checksum(&buffer[..34]);
            anyhow::ensure!(
                crc == ((buffer[34] as u16) << 8) | buffer[35] as u16,
                "CRC mismatch"
            );
            let flags = buffer[0];
            anyhow::ensure!(flags & 0x3f != 0x11, "Invalid flags");
            let flags = AddrFlags {
                testnet: flags & 0x80 != 0,
                bounceable: flags & 0x40 == 0,
            };

            Ok((
                flags,
                StdAddr::new(
                    buffer[1] as i8,
                    HashBytes(buffer[2..34].try_into().unwrap()),
                ),
            ))
        }

        let string = stack.pop_string()?;
        let (flags, addr) = 'addr: {
            if string.contains(':') {
                let flags = AddrFlags {
                    testnet: false,
                    bounceable: true,
                };
                if let Ok(addr) = StdAddr::from_str(&string) {
                    break 'addr (flags, addr);
                }
            } else if let Ok(addr) = unpack_base64_addr(&string) {
                break 'addr addr;
            };
            return stack.push_bool(false);
        };

        stack.push_int(addr.workchain)?;
        stack.push_int(BigInt::from_bytes_be(Sign::Plus, addr.address.as_slice()))?;
        stack.push_int(((flags.testnet as u8) << 1) + !flags.bounceable as u8)?;
        stack.push_bool(true)
    }
}
