use everscale_types::prelude::*;

use crate::dictionary::*;
use crate::error::*;
use crate::stack::*;

pub fn init(d: &mut Dictionary) -> FiftResult<()> {
    words!(d, {
        @stk "<b" => interpret_empty,
        @stk "b>" => |s| interpret_store_end(s, false),
        @stk "b>spec" => |s| interpret_store_end(s, true),
        @stk "<s" => interpret_from_cell,
        @stk "s>" => interpret_cell_check_empty,
        // TODO
    });
    Ok(())
}

fn interpret_empty(stack: &mut Stack) -> FiftResult<()> {
    stack.push(Box::new(CellBuilder::new()))
}

fn interpret_store_end(stack: &mut Stack, is_exotic: bool) -> FiftResult<()> {
    let mut item = stack.pop()?.into_builder()?;
    item.set_exotic(is_exotic);
    let cell = item.build()?;
    stack.push(Box::new(cell))
}

fn interpret_from_cell(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_cell()?;
    let slice = OwnedCellSlice::new(*item)?;
    stack.push(Box::new(slice))
}

fn interpret_cell_check_empty(stack: &mut Stack) -> FiftResult<()> {
    let item = stack.pop()?.into_slice()?;
    let item = item.as_ref().as_ref();
    if !item.is_data_empty() || !item.is_refs_empty() {
        return Err(FiftError::ExpectedEmptySlice);
    }
    Ok(())
}
