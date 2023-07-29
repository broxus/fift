use crate::context::*;
use crate::dictionary::*;
use crate::error::*;
use crate::stack::*;

pub fn init(d: &mut Dictionary) -> FiftResult<()> {
    words!(d, {
        // Stack print/dump words
        @ctx "." => |c| interpret_dot(c, true),
        @ctx "._" => |c| interpret_dot(c, false),
        @ctx "x." => |c| interpret_dothex(c, false, true),
        @ctx "x._" => |c| interpret_dothex(c, false, false),
        @ctx "X." => |c| interpret_dothex(c, true, true),
        @ctx "X._" => |c| interpret_dothex(c, true, false),
        @ctx "b." => |c| interpret_dotbin(c, true),
        @ctx "b._" => |c| interpret_dotbin(c, false),
        // TODO: csr.
        @ctx ".s" => interpret_dotstack,
        @ctx ".sl" => interpret_dotstack_list,
        @ctx ".dump" => interpret_dump,
        @ctx ".l" => interpret_print_list,
        @ctx ".bt" => interpret_print_backtrace,
        @ctx "cont." => interpret_print_continuation,
        @stk "(dump)" => interpret_dump_internal,
        @stk "(ldump)" => interpret_list_dump_internal,
        @stk "(.)" => interpret_dot_internal,
        @stk "(x.)" => |s| interpret_dothex_internal(s, false),
        @stk "(X.)" => |s| interpret_dothex_internal(s, true),
        @stk "(b.)" => interpret_dotbin_internal,
    });
    Ok(())
}

fn interpret_dot(ctx: &mut Context, space_after: bool) -> FiftResult<()> {
    let item = ctx.stack.pop()?.into_int()?;
    write!(ctx.stdout, "{item}{}", opt_space(space_after))?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dothex(ctx: &mut Context, uppercase: bool, space_after: bool) -> FiftResult<()> {
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

fn interpret_dotbin(ctx: &mut Context, space_after: bool) -> FiftResult<()> {
    let item = ctx.stack.pop()?.into_int()?;
    write!(ctx.stdout, "{:b}{}", item.as_ref(), opt_space(space_after))?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dotstack(ctx: &mut Context) -> FiftResult<()> {
    writeln!(ctx.stdout, "{}", ctx.stack.display_dump())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dotstack_list(ctx: &mut Context) -> FiftResult<()> {
    writeln!(ctx.stdout, "{}", ctx.stack.display_list())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dump(ctx: &mut Context) -> FiftResult<()> {
    let item = ctx.stack.pop()?;
    write!(ctx.stdout, "{} ", item.display_dump())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_print_list(ctx: &mut Context) -> FiftResult<()> {
    let item = ctx.stack.pop()?;
    write!(ctx.stdout, "{} ", item.display_list())?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_print_backtrace(ctx: &mut Context) -> FiftResult<()> {
    if let Some(next) = &ctx.next {
        writeln!(ctx.stdout, "{}", next.display_backtrace(&ctx.dictionary))?;
        ctx.stdout.flush()?;
    }
    Ok(())
}

fn interpret_print_continuation(ctx: &mut Context) -> FiftResult<()> {
    let cont = ctx.stack.pop()?.into_cont()?;
    writeln!(ctx.stdout, "{}", cont.display_backtrace(&ctx.dictionary))?;
    ctx.stdout.flush()?;
    Ok(())
}

fn interpret_dump_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?;
    stack.push(Box::new(item.display_dump().to_string()))
}

fn interpret_list_dump_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?;
    stack.push(Box::new(item.display_list().to_string()))
}

fn interpret_dot_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_int()?;
    stack.push(Box::new(item.to_string()))
}

fn interpret_dothex_internal(stack: &mut Stack, upper: bool) -> FiftResult<()> {
    let item = stack.pop()?.into_int()?;
    let item = if upper {
        format!("{:x}", item.as_ref())
    } else {
        format!("{:X}", item.as_ref())
    };
    stack.push(Box::new(item))
}

fn interpret_dotbin_internal(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_int()?;
    let item = format!("{:b}", item.as_ref());
    stack.push(Box::new(item))
}

const fn opt_space(space_after: bool) -> &'static str {
    if space_after {
        " "
    } else {
        ""
    }
}
