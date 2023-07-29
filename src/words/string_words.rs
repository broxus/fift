use crate::context::*;
use crate::dictionary::*;
use crate::error::*;

pub fn init(d: &mut Dictionary) -> FiftResult<()> {
    d.define_active_word("\"", interpret_quote_str)?;
    Ok(())
}

fn interpret_quote_str(ctx: &mut Context) -> FiftResult<()> {
    let word = ctx.input.scan_word_until('"')?;
    ctx.stack.push(Box::new(word.data.to_owned()))?;
    ctx.stack.push_argcount(1, ctx.dictionary.make_nop())
}
