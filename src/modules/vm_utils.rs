use std::fmt::Write;

use anyhow::Result;
use tycho_types::prelude::*;
use tycho_vm::{DumpOutput, DumpResult, GasParams, SafeRc, codepage0};

use crate::core::*;

pub struct VmUtils;

#[fift_module]
impl VmUtils {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        let vm_libraries = VM_LIBRARIES.with(SafeRc::clone);
        d.define_word(
            "vmlibs ",
            SafeRc::new(cont::LitCont(vm_libraries.into_dyn_fift_value())),
        )
    }

    #[cmd(name = "runvmx")]
    fn interpret_run_vm(ctx: &mut Context) -> Result<()> {
        let stack = &mut ctx.stack;

        // Pop VM mode and args.
        let mode = stack
            .pop_smallint_range(0, 0x7ff)
            .map(RunVmMode::from_bits_retain)?;

        let global_version = if mode.contains(RunVmMode::LOAD_GLOBAL_VERSION) {
            stack.pop_smallint_range(0, SUPPORTED_VERSION)?
        } else {
            SUPPORTED_VERSION
        };
        let mut gas_max = if mode.contains(RunVmMode::LOAD_GAS_MAX) {
            stack.pop_long_range(0, GasParams::MAX_GAS)?
        } else {
            GasParams::MAX_GAS
        };
        let gas_limit = if mode.contains(RunVmMode::LOAD_GAS_LIMIT) {
            stack.pop_long_range(0, GasParams::MAX_GAS)?
        } else {
            GasParams::MAX_GAS
        };

        if mode.contains(RunVmMode::LOAD_GAS_MAX) {
            gas_max = std::cmp::max(gas_max, gas_limit);
        } else {
            gas_max = gas_limit;
        };

        let gas = tycho_vm::GasParams {
            max: gas_max,
            limit: gas_limit,
            credit: 0,
            ..tycho_vm::GasParams::getter()
        };

        let c7 = if mode.contains(RunVmMode::LOAD_C7) {
            Some(stack.pop_tuple()?)
        } else {
            Default::default()
        };

        let data = if mode.contains(RunVmMode::LOAD_C4) {
            Some(stack.pop_cell().map(SafeRc::unwrap_or_clone)?)
        } else {
            None
        };

        let code = stack.pop_cell_slice().map(SafeRc::unwrap_or_clone)?;
        let libraries = VM_LIBRARIES.with(|libs| {
            SimpleLibraryProvider(Dict::from_raw(match libs.fetch().into_cell() {
                Ok(cell) => Some(SafeRc::unwrap_or_clone(cell)),
                Err(_) => None,
            }))
        });

        let mut vm = {
            let mut state = tycho_vm::VmState::builder()
                .with_smc_info(tycho_vm::CustomSmcInfo {
                    version: tycho_vm::VmVersion::Ton(global_version),
                    c7: Default::default(),
                })
                .with_data(data.clone().unwrap_or_default())
                .with_code(code)
                .with_libraries(&libraries)
                .with_gas(gas);
            if mode.contains(RunVmMode::SAME_C3) {
                state = state.with_init_selector(mode.contains(RunVmMode::PUSH_0));
            }
            state.build()
        };
        let res = !vm.run();

        let (data, actions) = if let Some(commited) = vm.committed_state {
            (Some(commited.c4), Some(commited.c5))
        } else {
            (data, None)
        };

        stack.push_int(res)?;
        if mode.contains(RunVmMode::LOAD_C4) {
            stack.push_opt(data)?;
        }
        if mode.contains(RunVmMode::RETURN_C5) {
            stack.push_opt(actions)?;
        }
        if mode.contains(RunVmMode::LOAD_GAS_LIMIT) {
            stack.push_int(vm.gas.consumed())?;
        }

        Ok(())
    }

    #[cmd(name = "vmcont,", stack)]
    fn interpret_store_vm_cont(stack: &mut Stack) -> Result<()> {
        let vmcont = stack.pop()?.into_vm_cont()?;
        let mut cb = stack.pop_builder()?;
        vmcont.store_into(SafeRc::make_mut(&mut cb), Cell::empty_context())?;
        stack.push_raw(cb.into_dyn_fift_value())
    }

    #[cmd(name = "vmcont@", stack)]
    fn interpret_fetch_vm_cont(stack: &mut Stack) -> Result<()> {
        let mut cs_raw = stack.pop_cell_slice()?;
        let mut cs = cs_raw.apply();

        let vmcont = tycho_vm::RcCont::load_from(&mut cs)?;

        let range = cs.range();
        SafeRc::make_mut(&mut cs_raw).set_range(range);

        stack.push_raw(cs_raw.into_dyn_fift_value())?;
        stack.push(vmcont)
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

    #[cmd(name = "supported-version", stack)]
    fn interpret_supported_version(stack: &mut Stack) -> Result<()> {
        stack.push_int(SUPPORTED_VERSION)
    }
}

thread_local! {
    static VM_LIBRARIES: SafeRc<SharedBox> = SafeRc::new(SharedBox::default());
}

#[repr(transparent)]
struct SimpleLibraryProvider(Dict<HashBytes, Cell>);

impl tycho_vm::LibraryProvider for SimpleLibraryProvider {
    fn find(&self, library_hash: &HashBytes) -> Result<Option<Cell>, tycho_types::error::Error> {
        let Ok(Some(lib)) = self.0.get(library_hash) else {
            return Ok(None);
        };
        Ok((lib.repr_hash() == library_hash).then_some(lib))
    }

    fn find_ref<'a>(
        &'a self,
        library_hash: &HashBytes,
    ) -> Result<Option<&'a DynCell>, tycho_types::error::Error> {
        let Ok(Some(mut cs)) = self.0.get_raw(library_hash) else {
            return Ok(None);
        };
        let Ok(lib) = cs.load_reference() else {
            return Ok(None);
        };
        Ok((lib.repr_hash() == library_hash).then_some(lib))
    }
}

const SUPPORTED_VERSION: u32 = const {
    match tycho_vm::VmVersion::LATEST_TON {
        tycho_vm::VmVersion::Ton(v) => v,
        tycho_vm::VmVersion::Everscale(_) => unreachable!(),
    }
};

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

bitflags::bitflags! {
    struct RunVmMode: u32 {
        // +1 Set c3 to code.
        const SAME_C3 = 0b0000_0000_0001;
        // +2 Push 0 on stack before running the code.
        const PUSH_0 = 0b0000_0000_0010;
        // +4 Load c4 from stack and return its final value.
        const LOAD_C4 = 0b0000_0000_0100;
        // +8 Load gas limit from stack and return consumed gas.
        const LOAD_GAS_LIMIT = 0b0000_0000_1000;
        // +16 Load smart-contract context into c7.
        const LOAD_C7 = 0b0000_0001_0000;
        // +32 Return c5 (actions).
        const RETURN_C5 = 0b0000_0010_0000;
        // +64 Log VM to stderr.
        const LOG_VM_OPS = 0b0000_0100_0000;
        // +128 Load hard gas limit from stack.
        const LOAD_GAS_MAX = 0b0000_1000_0000;
        // +256 Enable stack trace.
        const STACK_TRACE = 0b0001_0000_0000;
        // +512 Enable debug opcodes.
        const DEBUG_OPCODES = 0b0010_0000_0000;
        // +1024 Load global version from stack.
        const LOAD_GLOBAL_VERSION = 0b0100_0000_0000;
    }
}
