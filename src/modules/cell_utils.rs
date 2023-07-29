use everscale_types::prelude::*;

use crate::core::*;
use crate::error::*;

pub struct CellUtils;

#[fift_module]
impl CellUtils {
    #[cmd(name = "<b", stack)]
    fn interpret_empty(stack: &mut Stack) -> Result<()> {
        stack.push(Box::new(CellBuilder::new()))
    }

    #[cmd(name = "b>", stack, args(is_exotic = false))]
    #[cmd(name = "b>spec", stack, args(is_exotic = true))]
    fn interpret_store_end(stack: &mut Stack, is_exotic: bool) -> Result<()> {
        let mut item = stack.pop()?.into_builder()?;
        item.set_exotic(is_exotic);
        let cell = item.build()?;
        stack.push(Box::new(cell))
    }

    #[cmd(name = "<s", stack)]
    fn interpret_from_cell(stack: &mut Stack) -> Result<()> {
        let item = stack.pop()?.into_cell()?;
        let slice = OwnedCellSlice::new(*item)?;
        stack.push(Box::new(slice))
    }

    #[cmd(name = ">s", stack)]
    fn interpret_cell_check_empty(stack: &mut Stack) -> Result<()> {
        let item = stack.pop()?.into_slice()?;
        let item = item.as_ref().as_ref();
        if !item.is_data_empty() || !item.is_refs_empty() {
            return Err(Error::ExpectedEmptySlice);
        }
        Ok(())
    }
}
