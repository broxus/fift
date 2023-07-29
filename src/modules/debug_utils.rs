use crate::core::*;
use crate::error::*;

pub struct DebugUtils;

#[fift_module]
impl DebugUtils {
    #[cmd(name = ".", args(space_after = true))]
    #[cmd(name = "._", args(space_after = false))]
    fn interpret_dot(ctx: &mut Context, space_after: bool) -> Result<()> {
        let item = ctx.stack.pop()?.into_int()?;
        write!(ctx.stdout, "{item}{}", opt_space(space_after))?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "x.", args(uppercase = false, space_after = true))]
    #[cmd(name = "x._", args(uppercase = false, space_after = false))]
    #[cmd(name = "X.", args(uppercase = true, space_after = true))]
    #[cmd(name = "X._", args(uppercase = true, space_after = false))]
    fn interpret_dothex(ctx: &mut Context, uppercase: bool, space_after: bool) -> Result<()> {
        let item = ctx.stack.pop()?.into_int()?;
        let space = opt_space(space_after);
        if uppercase {
            write!(ctx.stdout, "{:X}{space}", item.as_ref())
        } else {
            write!(ctx.stdout, "{:x}{space}", item.as_ref())
        }?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "b.", args(space_after = true))]
    #[cmd(name = "b._", args(space_after = false))]
    fn interpret_dotbin(ctx: &mut Context, space_after: bool) -> Result<()> {
        let item = ctx.stack.pop()?.into_int()?;
        write!(ctx.stdout, "{:b}{}", item.as_ref(), opt_space(space_after))?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "Bx._")]
    fn interpret_bytes_hex_print_raw(ctx: &mut Context) -> Result<()> {
        const CHUNK: usize = 16;
        let bytes = ctx.stack.pop()?.into_bytes()?;
        let mut buffer: [u8; CHUNK * 2] = Default::default();
        for chunk in bytes.chunks(CHUNK) {
            let buffer = &mut buffer[..chunk.len() * 2];
            hex::encode_to_slice(chunk, buffer).unwrap();
            ctx.stdout.write_all(buffer)?;
        }
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = ".s")]
    fn interpret_dotstack(ctx: &mut Context) -> Result<()> {
        writeln!(ctx.stdout, "{}", ctx.stack.display_dump())?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = ".sl")]
    fn interpret_dotstack_list(ctx: &mut Context) -> Result<()> {
        writeln!(ctx.stdout, "{}", ctx.stack.display_list())?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = ".dump")]
    fn interpret_dump(ctx: &mut Context) -> Result<()> {
        let item = ctx.stack.pop()?;
        write!(ctx.stdout, "{} ", item.display_dump())?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = ".l")]
    fn interpret_print_list(ctx: &mut Context) -> Result<()> {
        let item = ctx.stack.pop()?;
        write!(ctx.stdout, "{} ", item.display_list())?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = ".bt")]
    fn interpret_print_backtrace(ctx: &mut Context) -> Result<()> {
        if let Some(next) = &ctx.next {
            writeln!(ctx.stdout, "{}", next.display_backtrace(&ctx.dictionary))?;
            ctx.stdout.flush()?;
        }
        Ok(())
    }

    #[cmd(name = "cont.")]
    fn interpret_print_continuation(ctx: &mut Context) -> Result<()> {
        let cont = ctx.stack.pop()?.into_cont()?;
        writeln!(ctx.stdout, "{}", cont.display_backtrace(&ctx.dictionary))?;
        ctx.stdout.flush()?;
        Ok(())
    }

    #[cmd(name = "(dump)", stack)]
    fn interpret_dump_internal(stack: &mut Stack) -> Result<()> {
        let item = stack.pop()?.display_dump().to_string();
        stack.push(Box::new(item))
    }

    #[cmd(name = "(ldump)", stack)]
    fn interpret_list_dump_internal(stack: &mut Stack) -> Result<()> {
        let item = stack.pop()?.display_list().to_string();
        stack.push(Box::new(item))
    }

    #[cmd(name = "(.)", stack)]
    fn interpret_dot_internal(stack: &mut Stack) -> Result<()> {
        let item = stack.pop()?.into_int()?;
        stack.push(Box::new(item.to_string()))
    }

    #[cmd(name = "(x.)", stack, args(upper = false))]
    #[cmd(name = "(X.)", stack, args(upper = true))]
    fn interpret_dothex_internal(stack: &mut Stack, upper: bool) -> Result<()> {
        let item = stack.pop()?.into_int()?;
        let item = if upper {
            format!("{:x}", item.as_ref())
        } else {
            format!("{:X}", item.as_ref())
        };
        stack.push(Box::new(item))
    }

    #[cmd(name = "(b.)", stack)]
    fn interpret_dotbin_internal(stack: &mut Stack) -> Result<()> {
        let item = stack.pop()?.into_int()?;
        let item = format!("{:b}", item.as_ref());
        stack.push(Box::new(item))
    }
}

const fn opt_space(space_after: bool) -> &'static str {
    if space_after {
        " "
    } else {
        ""
    }
}
