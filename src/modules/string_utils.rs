use num_bigint::BigInt;
use num_traits::Num;

use crate::core::*;
use crate::error::*;
use crate::util::*;

pub struct StringUtils;

#[fift_module]
impl StringUtils {
    #[cmd(name = "\"", active, without_space)]
    fn interpret_quote_str(ctx: &mut Context) -> Result<()> {
        let word = ctx.input.scan_word_until('"')?;
        ctx.stack.push(Box::new(word.data.to_owned()))?;
        ctx.stack.push_argcount(1, ctx.dictionary.make_nop())
    }

    #[cmd(name = "char", active)]
    fn interpret_char(ctx: &mut Context) -> Result<()> {
        let token = ctx.input.scan_word()?.ok_or(Error::UnexpectedEof)?;
        let mut chars = token.data.chars();
        let char = chars.next().ok_or(Error::UnexpectedEof)?;
        if chars.next().is_some() {
            return Err(Error::InvalidChar);
        }
        ctx.stack.push_smallint(char as u32)?;
        ctx.stack.push_argcount(1, ctx.dictionary.make_nop())
    }

    #[cmd(name = "(char)", stack)]
    fn interpret_char_internal(stack: &mut Stack) -> Result<()> {
        let string = stack.pop()?.into_string()?;
        let mut chars = string.chars();
        let char = chars.next().ok_or(Error::UnexpectedEof)?;
        if chars.next().is_some() {
            return Err(Error::InvalidChar);
        }
        stack.push_smallint(char as u32)
    }

    #[cmd(name = "emit")]
    fn interpret_emit(ctx: &mut Context) -> Result<()> {
        let c = ctx.stack.pop_smallint_char()?;
        write!(ctx.stdout, "{c}")?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "space", args(c = ' '))]
    #[cmd(name = "cr", args(c = '\n'))]
    fn interpret_emit_const(ctx: &mut Context, c: char) -> Result<()> {
        write!(ctx.stdout, "{c}")?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "type")]
    fn interpret_type(ctx: &mut Context) -> Result<()> {
        let string = ctx.stack.pop()?.into_string()?;
        write!(ctx.stdout, "{string}")?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "string?", stack)]
    fn interpret_is_string(stack: &mut Stack) -> Result<()> {
        let is_string = stack.pop()?.ty() == StackValueType::String;
        stack.push_bool(is_string)
    }

    #[cmd(name = "chr", stack)]
    fn interpret_chr(stack: &mut Stack) -> Result<()> {
        let c = stack.pop_smallint_char()?;
        stack.push(Box::new(c.to_string()))
    }

    #[cmd(name = "hold", stack)]
    fn interpret_hold(stack: &mut Stack) -> Result<()> {
        let c = stack.pop_smallint_char()?;
        let mut string = stack.pop()?.into_string()?;
        string.push(c);
        stack.push(string)
    }

    #[cmd(name = "(number)", stack)]
    fn interpret_parse_number(stack: &mut Stack) -> Result<()> {
        let string = stack.pop()?.into_string()?;
        let mut res = 0;
        if let Ok(Some(int)) = ImmediateInt::try_from_str(&string) {
            res += 1;
            stack.push(Box::new(int.num))?;
            if let Some(denom) = int.denom {
                res += 1;
                stack.push(Box::new(denom))?;
            }
        }
        stack.push_smallint(res)
    }

    #[cmd(name = "(hex-number)", stack)]
    fn interpret_parse_hex_number(stack: &mut Stack) -> Result<()> {
        let string = stack.pop()?.into_string()?;
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
            stack.push(Box::new(num))?;
        }
        stack.push_smallint(res)
    }

    #[cmd(name = "$|", stack)]
    #[cmd(name = "$Split", stack)]
    fn interpret_str_split(stack: &mut Stack) -> Result<()> {
        let at = stack.pop_smallint_range(0, i32::MAX as _)? as usize;
        let mut head = stack.pop()?.into_string()?;
        if at > head.len() || !head.is_char_boundary(at) {
            return Err(Error::InvalidIndex);
        }
        let tail = Box::new(head[at..].to_owned());
        head.truncate(at);

        stack.push(head)?;
        stack.push(tail)
    }

    #[cmd(name = "$+", stack)]
    fn interpret_str_concat(stack: &mut Stack) -> Result<()> {
        let tail = stack.pop()?.into_string()?;
        let mut head = stack.pop()?.into_string()?;
        head.push_str(&tail);
        stack.push(head)
    }

    #[cmd(name = "$=", stack)]
    fn interpret_str_equal(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop()?.into_string()?;
        let rhs = stack.pop()?.into_string()?;
        stack.push_bool(lhs == rhs)
    }

    #[cmd(name = "$cmp", stack)]
    fn interpret_str_cmp(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop()?.into_string()?;
        let rhs = stack.pop()?.into_string()?;
        stack.push(Box::new(BigInt::from(match lhs.cmp(&rhs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        })))
    }

    #[cmd(name = "$reverse", stack)]
    fn interpret_str_reverse(stack: &mut Stack) -> Result<()> {
        let mut string = stack.pop()?.into_string()?;
        reverse_utf8_string_inplace(string.as_mut_str());
        stack.push(string)
    }

    #[cmd(name = "$pos", stack)]
    #[cmd(name = "$Pos", stack)]
    fn interpret_str_pos(stack: &mut Stack) -> Result<()> {
        let substring = stack.pop()?.into_string()?;
        let string = stack.pop()?.into_string()?;
        stack.push(Box::new(match string.find(substring.as_str()) {
            Some(idx) => BigInt::from(idx),
            None => BigInt::from(-1),
        }))
    }

    #[cmd(name = "(-trailing)", stack, args(arg = None))]
    #[cmd(name = "-trailing", stack, args(arg = Some(' ')))]
    #[cmd(name = "-trailing0", stack, args(arg = Some('0')))]
    fn interpret_str_remove_trailing_int(stack: &mut Stack, arg: Option<char>) -> Result<()> {
        let arg = match arg {
            Some(arg) => arg,
            None => stack.pop_smallint_char()?,
        };
        let mut string = stack.pop()?.into_string()?;
        string.truncate(string.trim_end_matches(arg).len());
        stack.push(string)
    }

    #[cmd(name = "$len", stack)]
    fn interpret_str_len(stack: &mut Stack) -> Result<()> {
        let len = stack.pop()?.as_string()?.len();
        stack.push(Box::new(BigInt::from(len)))
    }

    #[cmd(name = "Blen", stack)]
    fn interpret_bytes_len(stack: &mut Stack) -> Result<()> {
        let len = stack.pop()?.as_bytes()?.len();
        stack.push(Box::new(BigInt::from(len)))
    }

    #[cmd(name = "$Len", stack)]
    fn interpret_utf8_str_len(stack: &mut Stack) -> Result<()> {
        let string = stack.pop()?.into_string()?;
        let len = string.chars().count();
        stack.push(Box::new(BigInt::from(len)))
    }

    #[cmd(name = "B>X", stack, args(upper = true))]
    #[cmd(name = "B>x", stack, args(upper = false))]
    fn interpret_bytes_to_hex(stack: &mut Stack, upper: bool) -> Result<()> {
        let bytes = stack.pop()?.into_bytes()?;
        let string = if upper {
            hex::encode_upper(*bytes)
        } else {
            hex::encode(*bytes)
        };
        stack.push(Box::new(string))
    }

    #[cmd(name = "x>B", stack, args(partial = false))]
    #[cmd(name = "x>B?", stack, args(partial = true))]
    fn interpret_hex_to_bytes(stack: &mut Stack, partial: bool) -> Result<()> {
        let mut string = stack.pop()?.into_string()?;
        if partial {
            let len = string
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(string.len())
                & (usize::MAX - 1);
            string.truncate(len);
        }

        let i = string.len();
        let bytes = hex::decode(*string).map_err(|_| Error::InvalidString)?;

        stack.push(Box::new(bytes))?;
        if partial {
            stack.push(Box::new(BigInt::from(i)))?;
        }
        Ok(())
    }

    #[cmd(name = "B|", stack)]
    fn interpret_bytes_split(stack: &mut Stack) -> Result<()> {
        let at = stack.pop_smallint_range(0, i32::MAX as _)? as usize;
        let mut head = stack.pop()?.into_bytes()?;
        if at > head.len() {
            return Err(Error::InvalidIndex);
        }
        let tail = Box::new(head[at..].to_owned());
        head.truncate(at);

        stack.push(head)?;
        stack.push(tail)
    }

    #[cmd(name = "B+", stack)]
    fn interpret_bytes_concat(stack: &mut Stack) -> Result<()> {
        let tail = stack.pop()?.into_bytes()?;
        let mut head = stack.pop()?.into_bytes()?;
        head.extend_from_slice(&tail);
        stack.push(head)
    }

    #[cmd(name = "B=", stack)]
    fn interpret_bytes_equal(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop()?.into_bytes()?;
        let rhs = stack.pop()?.into_bytes()?;
        stack.push_bool(lhs == rhs)
    }

    #[cmd(name = "Bcmp", stack)]
    fn interpret_bytes_cmp(stack: &mut Stack) -> Result<()> {
        let lhs = stack.pop()?.into_bytes()?;
        let rhs = stack.pop()?.into_bytes()?;
        stack.push(Box::new(BigInt::from(match lhs.cmp(&rhs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        })))
    }
}
