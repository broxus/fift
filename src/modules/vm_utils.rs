use std::fmt::Write;

use anyhow::Result;
use tycho_types::prelude::*;
use tycho_vm::{codepage0, DumpOutput, DumpResult, SafeRc};

use crate::core::*;

pub struct VmUtils;

#[fift_module]
impl VmUtils {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        thread_local! {
            static VM_LIBRARIES: SafeRc<dyn StackValue> = SafeRc::new_dyn_fift_value(SharedBox::default());
        }

        let vm_libraries = VM_LIBRARIES.with(|b| b.clone());

        d.define_word("vmlibs ", SafeRc::new(cont::LitCont(vm_libraries)))?;

        Ok(())
    }

    #[cmd(name = "dbrunvm")]
    #[cmd(name = "dbrunvm-parallel")]
    #[cmd(name = "vmcont")]
    #[cmd(name = "vmcont@")]
    #[cmd(name = "runvmx")]
    fn interpret_run_vm(_ctx: &mut Context) -> Result<()> {
        anyhow::bail!("Unimplemented");
    }

    #[cmd(name = "(vmoplen)", stack)]
    fn interpret_vmop_len(stack: &mut Stack) -> Result<()> {
        let cp = stack.pop_smallint_signed_range(i32::MIN, i32::MAX)?;
        anyhow::ensure!(cp == 0, "Unknown VM codepage");

        let cs_raw = stack.pop_cell_slice()?;
        let mut cs = cs_raw.apply();

        let before = cs.offset();
        codepage0()
            .dispatch_dump(&mut cs, &mut NoopDump)
            .unwrap_or_default();
        let size = cs.offset() - before;

        stack.push_int(((size.refs as u64) << 16) | (size.bits as u64))
    }

    #[cmd(name = "(vmopdump)", stack)]
    fn interpret_vmopdump(stack: &mut Stack) -> Result<()> {
        let cp = stack.pop_smallint_signed_range(i32::MIN, i32::MAX)?;
        anyhow::ensure!(cp == 0, "Unknown VM codepage");

        let mut cs_raw = stack.pop_cell_slice()?;
        let mut cs = cs_raw.apply();

        let mut dump = OpcodeNameDump(String::new());
        codepage0().dispatch_dump(&mut cs, &mut dump)?;

        let range = cs.range();
        SafeRc::make_mut(&mut cs_raw).set_range(range);

        stack.push_raw(cs_raw.into_dyn_fift_value())?;
        stack.push(dump.0)
    }
}

struct NoopDump;

impl DumpOutput for NoopDump {
    fn record_gas(&mut self, _: u64) -> DumpResult {
        Ok(())
    }

    fn record_opcode(&mut self, _: &dyn std::fmt::Display) -> DumpResult {
        Ok(())
    }

    fn record_cell(&mut self, _: Cell) -> DumpResult {
        Ok(())
    }

    fn record_slice(&mut self, _: CellSlice<'_>) -> DumpResult {
        Ok(())
    }

    fn record_cont(&mut self, _: Cell) -> DumpResult {
        Ok(())
    }

    fn record_cont_slice(&mut self, _: CellSlice<'_>) -> DumpResult {
        Ok(())
    }

    fn record_dict(&mut self, _: u16, _: CellSlice<'_>) -> DumpResult {
        Ok(())
    }
}

struct OpcodeNameDump(String);

impl DumpOutput for OpcodeNameDump {
    fn record_gas(&mut self, _: u64) -> DumpResult {
        Ok(())
    }

    fn record_opcode(&mut self, opcode: &dyn std::fmt::Display) -> DumpResult {
        write!(&mut self.0, "{}", opcode)?;
        Ok(())
    }

    fn record_cell(&mut self, _: Cell) -> DumpResult {
        Ok(())
    }

    fn record_slice(&mut self, _: CellSlice<'_>) -> DumpResult {
        Ok(())
    }

    fn record_cont(&mut self, _: Cell) -> DumpResult {
        Ok(())
    }

    fn record_cont_slice(&mut self, _: CellSlice<'_>) -> DumpResult {
        Ok(())
    }

    fn record_dict(&mut self, _: u16, _: CellSlice<'_>) -> DumpResult {
        Ok(())
    }
}
