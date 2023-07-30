use everscale_types::prelude::*;
use num_traits::ToPrimitive;

use crate::core::*;
use crate::error::*;

pub struct CellUtils;

#[fift_module]
impl CellUtils {
    #[cmd(name = "<b", stack)]
    fn interpret_empty(stack: &mut Stack) -> Result<()> {
        stack.push(CellBuilder::new())
    }

    #[cmd(name = "i,", stack, args(signed = true))]
    #[cmd(name = "u,", stack, args(signed = false))]
    fn interpret_store(stack: &mut Stack, signed: bool) -> Result<()> {
        let bits = stack.pop_smallint_range(0, 1023)? as u16;
        let mut int = stack.pop_int()?;
        let mut builder = stack.pop_builder()?;

        if int.bits() > bits as u64 {
            return Err(Error::IntegerOverflow);
        }

        match int.to_u64() {
            Some(value) => builder.store_uint(value, bits)?,
            None => {
                if bits % 8 != 0 {
                    let align = 8 - bits % 8;
                    *int <<= align;
                }

                let minimal_bytes = ((bits + 7) / 8) as usize;

                let (prefix, mut bytes) = if signed {
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
                bytes.resize(minimal_bytes, prefix);
                bytes.reverse();

                builder.store_raw(&bytes, bits)?;
            }
        };

        stack.push_raw(builder)
    }

    #[cmd(name = "ref", stack)]
    fn interpret_store_ref(stack: &mut Stack) -> Result<()> {
        let cell = stack.pop_cell()?;
        let mut builder = stack.pop_builder()?;
        builder.store_reference(*cell)?;
        stack.push_raw(builder)
    }

    #[cmd(name = "$,", stack)]
    fn interpret_store_str(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let mut builder = stack.pop_builder()?;
        builder.store_raw(string.as_bytes(), len_as_bits(&*string)?)?;
        stack.push_raw(builder)
    }

    #[cmd(name = "B,", stack)]
    fn interpret_store_bytes(stack: &mut Stack) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let mut builder = stack.pop_builder()?;
        builder.store_raw(bytes.as_slice(), len_as_bits(&*bytes)?)?;
        stack.push_raw(builder)
    }

    #[cmd(name = "s,", stack)]
    fn interpret_store_cellslice(stack: &mut Stack) -> Result<()> {
        let slice = stack.pop_slice()?;
        let mut builder = stack.pop_builder()?;
        builder.store_slice(OwnedCellSlice::as_ref(&slice))?;
        stack.push_raw(builder)
    }

    #[cmd(name = "sr,", stack)]
    fn interpret_store_cellslice_ref(stack: &mut Stack) -> Result<()> {
        let slice = stack.pop_slice()?;
        let cell = {
            let mut builder = CellBuilder::new();
            builder.store_slice(OwnedCellSlice::as_ref(&slice))?;
            builder.build()?
        };
        let mut builder = stack.pop_builder()?;
        builder.store_reference(cell)?;
        stack.push_raw(builder)
    }

    #[cmd(name = "b>", stack, args(is_exotic = false))]
    #[cmd(name = "b>spec", stack, args(is_exotic = true))]
    fn interpret_store_end(stack: &mut Stack, is_exotic: bool) -> Result<()> {
        let mut item = stack.pop_builder()?;
        item.set_exotic(is_exotic);
        let cell = item.build()?;
        stack.push(cell)
    }

    #[cmd(name = "$>s", stack)]
    fn interpret_string_to_cellslice(stack: &mut Stack) -> Result<()> {
        let string = stack.pop_string()?;
        let mut builder = CellBuilder::new();
        builder.store_raw(string.as_bytes(), len_as_bits(&*string)?)?;
        let slice = OwnedCellSlice::new(builder.build()?)?;
        stack.push(slice)
    }

    #[cmd(name = "<s", stack)]
    fn interpret_from_cell(stack: &mut Stack) -> Result<()> {
        let item = stack.pop_cell()?;
        let slice = OwnedCellSlice::new(*item)?;
        stack.push(slice)
    }

    #[cmd(name = ">s", stack)]
    fn interpret_cell_check_empty(stack: &mut Stack) -> Result<()> {
        let item = stack.pop_slice()?;
        let item = item.as_ref().as_ref();
        if !item.is_data_empty() || !item.is_refs_empty() {
            return Err(Error::ExpectedEmptySlice);
        }
        Ok(())
    }

    #[cmd(name = "B>boc", stack)]
    fn interpret_boc_deserialize(stack: &mut Stack) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let cell = Boc::decode(*bytes)?;
        stack.push(cell)
    }

    #[cmd(name = "boc>B", stack)]
    fn interpret_boc_serialize(stack: &mut Stack) -> Result<()> {
        let cell = stack.pop_cell()?;
        let bytes = Boc::encode(*cell);
        stack.push(bytes)
    }

    #[cmd(name = "boc>base64", stack)]
    fn interpret_boc_serialize_base64(stack: &mut Stack) -> Result<()> {
        let cell = stack.pop_cell()?;
        let string = Boc::encode_base64(*cell);
        stack.push(string)
    }
}

fn len_as_bits<T: AsRef<[u8]>>(data: T) -> Result<u16> {
    let bits = data.as_ref().len() * 8;
    if bits > everscale_types::cell::MAX_BIT_LEN as usize {
        return Err(Error::CellError(
            everscale_types::error::Error::CellOverflow,
        ));
    }
    Ok(bits as u16)
}
