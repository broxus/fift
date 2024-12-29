use anyhow::Result;

use crate::core::*;
use crate::util::*;

pub struct DebugUtils;

#[fift_module]
impl DebugUtils {
    #[cmd(name = ".", args(space_after = true))]
    #[cmd(name = "._", args(space_after = false))]
    fn interpret_dot(ctx: &mut Context, space_after: bool) -> Result<()> {
        let int = ctx.stack.pop_int()?;
        write!(ctx.stdout, "{int}{}", opt_space(space_after))?;
        Ok(())
    }

    #[cmd(name = "x.", args(uppercase = false, space_after = true))]
    #[cmd(name = "x._", args(uppercase = false, space_after = false))]
    #[cmd(name = "X.", args(uppercase = true, space_after = true))]
    #[cmd(name = "X._", args(uppercase = true, space_after = false))]
    fn interpret_dothex(ctx: &mut Context, uppercase: bool, space_after: bool) -> Result<()> {
        let int = ctx.stack.pop_int()?;
        let space = opt_space(space_after);
        if uppercase {
            write!(ctx.stdout, "{:X}{space}", int.as_ref())
        } else {
            write!(ctx.stdout, "{:x}{space}", int.as_ref())
        }?;
        Ok(())
    }

    #[cmd(name = "b.", args(space_after = true))]
    #[cmd(name = "b._", args(space_after = false))]
    fn interpret_dotbin(ctx: &mut Context, space_after: bool) -> Result<()> {
        let int = ctx.stack.pop_int()?;
        write!(ctx.stdout, "{:b}{}", int.as_ref(), opt_space(space_after))?;
        Ok(())
    }

    #[cmd(name = "csr.", args(pop_limit = false))]
    #[cmd(name = "lcsr.", args(pop_limit = true))]
    fn interpret_dot_cellslice_rec(ctx: &mut Context, pop_limit: bool) -> Result<()> {
        const DEFAULT_RECURSIVE_PRINT_LIMIT: usize = 100;

        let limit = if pop_limit {
            ctx.stack.pop_smallint_range(0, u16::MAX as u32)? as usize
        } else {
            DEFAULT_RECURSIVE_PRINT_LIMIT
        };

        let cs = ctx.stack.pop_cell_slice()?;
        write!(ctx.stdout, "{}", cs.apply().display_slice_tree(limit))?;
        Ok(())
    }

    #[cmd(name = "Bx.")]
    fn interpret_bytes_hex_print_raw(ctx: &mut Context) -> Result<()> {
        const CHUNK: usize = 16;
        let bytes = ctx.stack.pop_bytes()?;
        let mut buffer: [u8; CHUNK * 2] = Default::default();
        for chunk in bytes.chunks(CHUNK) {
            let buffer = &mut buffer[..chunk.len() * 2];
            hex::encode_to_slice(chunk, buffer).unwrap();
            ctx.stdout.write_all(buffer)?;
        }
        Ok(())
    }

    #[cmd(name = ".s")]
    fn interpret_dotstack(ctx: &mut Context) -> Result<()> {
        writeln!(ctx.stdout, "{}", ctx.stack.display_dump())?;
        Ok(())
    }

    #[cmd(name = ".sl")]
    fn interpret_dotstack_list(ctx: &mut Context) -> Result<()> {
        writeln!(ctx.stdout, "{}", ctx.stack.display_list())?;
        Ok(())
    }

    #[cmd(name = ".dump")]
    fn interpret_dump(ctx: &mut Context) -> Result<()> {
        let item = ctx.stack.pop()?;
        write!(ctx.stdout, "{} ", item.display_dump())?;
        Ok(())
    }

    #[cmd(name = ".l")]
    fn interpret_print_list(ctx: &mut Context) -> Result<()> {
        let item = ctx.stack.pop()?;
        write!(ctx.stdout, "{} ", item.display_list())?;
        Ok(())
    }

    #[cmd(name = ".bt")]
    fn interpret_print_backtrace(ctx: &mut Context) -> Result<()> {
        if let Some(next) = &ctx.next {
            writeln!(ctx.stdout, "{}", next.display_backtrace(&ctx.dicts.current))?;
        }
        Ok(())
    }

    #[cmd(name = "cont.")]
    fn interpret_print_continuation(ctx: &mut Context) -> Result<()> {
        let cont = ctx.stack.pop_cont()?;
        writeln!(ctx.stdout, "{}", cont.display_backtrace(&ctx.dicts.current))?;
        Ok(())
    }

    #[cmd(name = "(dump)", stack)]
    fn interpret_dump_internal(stack: &mut Stack) -> Result<()> {
        let string = stack.pop()?.display_dump().to_string();
        stack.push(string)
    }

    #[cmd(name = "(ldump)", stack)]
    fn interpret_list_dump_internal(stack: &mut Stack) -> Result<()> {
        let string = stack.pop()?.display_list().to_string();
        stack.push(string)
    }

    #[cmd(name = "(.)", stack)]
    fn interpret_dot_internal(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_int()?.to_string();
        stack.push(string)
    }

    #[cmd(name = "(x.)", stack, args(upper = false))]
    #[cmd(name = "(X.)", stack, args(upper = true))]
    fn interpret_dothex_internal(stack: &mut Stack, upper: bool) -> Result<()> {
        let int = stack.pop_int()?;
        let string = if upper {
            format!("{:x}", int.as_ref())
        } else {
            format!("{:X}", int.as_ref())
        };
        stack.push(string)
    }

    #[cmd(name = "(b.)", stack)]
    fn interpret_dotbin_internal(stack: &mut Stack) -> Result<()> {
        let int = stack.pop_int()?;
        let string = format!("{:b}", int.as_ref());
        stack.push(string)
    }

    #[cmd(name = "words")]
    fn interpret_words(ctx: &mut Context) -> Result<()> {
        let Some(map) = ctx.dicts.current.clone_words_map()? else {
            return Ok(());
        };

        let mut all_words = map
            .as_ref()
            .into_iter()
            .map(|entry| entry.key.stack_value.as_string())
            .collect::<Result<Vec<_>>>()?;
        all_words.sort();

        let mut first = true;
        for word in all_words {
            let space = if std::mem::take(&mut first) { "" } else { " " };
            write!(ctx.stdout, "{space}{word}")?;
        }
        Ok(())
    }
}

const fn opt_space(space_after: bool) -> &'static str {
    if space_after {
        " "
    } else {
        ""
    }
}
