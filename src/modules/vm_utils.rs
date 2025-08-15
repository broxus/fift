use std::cell::RefCell;
use std::fmt::Write;
use std::num::NonZeroU64;
use std::rc::Rc;

use anyhow::Result;
use tracing::span;
use tycho_types::prelude::*;
use tycho_vm::{DumpOutput, DumpResult, GasParams, SafeRc, VmLogMask, codepage0};

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
            None
        };
        let smc_info = tycho_vm::CustomSmcInfo {
            version: tycho_vm::VmVersion::Ton(global_version),
            c7: match c7 {
                Some(tuple) => {
                    SafeRc::from(Rc::new(fift_rc_tuple_to_vm(SafeRc::into_inner(tuple))))
                }
                None => Default::default(),
            },
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

        let stderr = IoWriter(Rc::new(RefCell::new(ctx.stderr)));

        let mut log_mask = VmLogMask::empty();
        if mode.contains(RunVmMode::LOG_VM_OPS) {
            log_mask |= VmLogMask::MESSAGE;
            if mode.contains(RunVmMode::STACK_TRACE) {
                log_mask |= VmLogMask::DUMP_STACK;
            }
        }

        let mut debug_writer;
        let mut vm = {
            let mut state = tycho_vm::VmState::builder()
                .with_smc_info(smc_info)
                .with_data(data.clone().unwrap_or_default())
                .with_code(code)
                .with_raw_stack(SafeRc::new(tycho_vm::Stack {
                    items: fift_tuple_to_vm(stack.take_items()),
                }))
                .with_libraries(&libraries)
                .with_modifiers(tycho_vm::BehaviourModifiers {
                    log_mask,
                    ..Default::default()
                })
                .with_gas(gas);
            if mode.contains(RunVmMode::SAME_C3) {
                state = state.with_init_selector(mode.contains(RunVmMode::PUSH_0));
            }
            if mode.contains(RunVmMode::DEBUG_OPCODES) {
                debug_writer = stderr.clone();
                state = state.with_debug(&mut debug_writer);
            }
            state.build()
        };

        let res = tracing::subscriber::with_default(
            VmLogSubscriber {
                // NOTE: Forbidden crimes.
                writer: unsafe { std::mem::transmute::<IoWriter<'_>, IoWriter<'static>>(stderr) },
                log_mask,
            },
            || !vm.run(),
        );

        stack.set_items({
            let stack = SafeRc::into_inner(std::mem::take(&mut vm.stack));
            match Rc::try_unwrap(stack) {
                Ok(stack) => stack.items.into_iter().map(vm_value_to_fift).collect(),
                Err(stack) => stack
                    .items
                    .iter()
                    .map(|item| vm_value_to_fift(item.clone()))
                    .collect(),
            }
        });

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

fn fift_rc_tuple_to_vm(items: Rc<Vec<SafeRc<dyn StackValue>>>) -> Vec<tycho_vm::RcStackValue> {
    match Rc::try_unwrap(items) {
        Ok(items) => items.into_iter().map(fift_value_to_vm).collect(),
        Err(items) => items
            .iter()
            .map(|item| fift_value_to_vm(item.clone()))
            .collect(),
    }
}

fn fift_tuple_to_vm(items: Vec<SafeRc<dyn StackValue>>) -> Vec<tycho_vm::RcStackValue> {
    items.into_iter().map(fift_value_to_vm).collect()
}

fn fift_value_to_vm(item: SafeRc<dyn StackValue>) -> tycho_vm::RcStackValue {
    match item.ty() {
        StackValueType::Null => tycho_vm::Stack::make_null(),
        StackValueType::NaN => tycho_vm::Stack::make_nan(),
        StackValueType::Int => SafeRc::from(SafeRc::into_inner(item).rc_into_int().unwrap()),
        StackValueType::Cell => SafeRc::from(SafeRc::into_inner(item).rc_into_cell().unwrap()),
        StackValueType::Builder => {
            SafeRc::from(SafeRc::into_inner(item).rc_into_builder().unwrap())
        }
        StackValueType::Slice => {
            SafeRc::from(SafeRc::into_inner(item).rc_into_cell_slice().unwrap())
        }
        StackValueType::Tuple => SafeRc::from(Rc::new(fift_rc_tuple_to_vm(
            SafeRc::into_inner(item).rc_into_tuple().unwrap(),
        ))),
        StackValueType::VmCont => {
            SafeRc::from(SafeRc::into_inner(item.into_vm_cont().unwrap()).rc_into_dyn())
        }
        // TODO: Somehow remove alloc here.
        _ => SafeRc::from(Rc::new(CustomFiftValue(item))),
    }
}

fn vm_rc_tuple_to_fift(items: Rc<Vec<tycho_vm::RcStackValue>>) -> Vec<SafeRc<dyn StackValue>> {
    match Rc::try_unwrap(items) {
        Ok(items) => items.into_iter().map(vm_value_to_fift).collect(),
        Err(items) => items
            .iter()
            .map(|item| vm_value_to_fift(item.clone()))
            .collect(),
    }
}

fn vm_value_to_fift(item: tycho_vm::RcStackValue) -> SafeRc<dyn StackValue> {
    if let Some(ty) = tycho_vm::StackValueType::from_raw(item.raw_ty()) {
        return match ty {
            tycho_vm::StackValueType::Null => Stack::make_null(),
            tycho_vm::StackValueType::Int => match item.into_int() {
                Ok(item) => item.into_dyn_fift_value(),
                Err(_) => Stack::make_nan(),
            },
            tycho_vm::StackValueType::Cell => item.into_cell().unwrap().into_dyn_fift_value(),
            tycho_vm::StackValueType::Slice => {
                item.into_cell_slice().unwrap().into_dyn_fift_value()
            }
            tycho_vm::StackValueType::Builder => {
                item.into_cell_builder().unwrap().into_dyn_fift_value()
            }
            tycho_vm::StackValueType::Cont => SafeRc::new_dyn_fift_value(item.into_cont().unwrap()),
            tycho_vm::StackValueType::Tuple => SafeRc::new_dyn_fift_value(vm_rc_tuple_to_fift(
                SafeRc::into_inner(item.into_tuple().unwrap()),
            )),
        };
    }

    let ptr = item.as_ptr();
    let [_, vtable] =
        unsafe { std::mem::transmute::<*const dyn tycho_vm::StackValue, [*const (); 2]>(ptr) };

    if vtable == CustomFiftValue::VTABLE_PTR {
        let value = Rc::into_raw(SafeRc::into_inner(item)).cast::<CustomFiftValue>();
        let value = Rc::unwrap_or_clone(unsafe { Rc::from_raw(value) });
        return value.0;
    }

    unreachable!()
}

struct VmLogSubscriber {
    log_mask: tycho_vm::VmLogMask,
    writer: IoWriter<'static>,
}

// NOTE: Forbidden crimes
unsafe impl Send for VmLogSubscriber {}
unsafe impl Sync for VmLogSubscriber {}

impl tracing::Subscriber for VmLogSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target() == tycho_vm::VM_LOG_TARGET
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_non_zero_u64(NonZeroU64::MIN)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }

        event.record(&mut LogVisitor {
            writer: &mut *self.writer.0.borrow_mut(),
            log_mask: self.log_mask,
        });
    }

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

struct LogVisitor<'a> {
    log_mask: VmLogMask,
    writer: &'a mut dyn std::fmt::Write,
}

impl tracing::field::Visit for LogVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        const STACK_MASK: VmLogMask = VmLogMask::DUMP_STACK.union(VmLogMask::DUMP_STACK_VERBOSE);

        let res = match field.name() {
            "message" if self.log_mask.contains(VmLogMask::MESSAGE) => {
                writeln!(self.writer, "{value:?}")
            }
            "opcode" if self.log_mask.contains(VmLogMask::MESSAGE) => {
                writeln!(self.writer, "execute {value:?}")
            }
            "stack" if self.log_mask.intersects(STACK_MASK) => {
                writeln!(self.writer, "stack: {value:?}")
            }
            "exec_location" if self.log_mask.contains(VmLogMask::EXEC_LOCATION) => {
                writeln!(self.writer, "code cell hash: {value:?}")
            }
            "gas_remaining" if self.log_mask.contains(VmLogMask::GAS_REMAINING) => {
                writeln!(self.writer, "gas remaining: {value:?}")
            }
            "c5" if self.log_mask.contains(VmLogMask::DUMP_C5) => {
                writeln!(self.writer, "c5: {value:?}")
            }
            _ => return,
        };
        res.unwrap();
    }
}

#[derive(Clone)]
struct IoWriter<'a>(Rc<RefCell<&'a mut dyn std::fmt::Write>>);

impl std::fmt::Write for IoWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0
            .borrow_mut()
            .write_str(s)
            .map_err(|_| std::fmt::Error)
    }
}

#[derive(Clone)]
#[repr(transparent)]
struct CustomFiftValue(SafeRc<dyn StackValue>);

impl CustomFiftValue {
    const VTABLE_PTR: *const () = const {
        let [_, vtable] = unsafe {
            std::mem::transmute::<*const dyn tycho_vm::StackValue, [*const (); 2]>(
                std::ptr::null::<CustomFiftValue>(),
            )
        };
        vtable
    };
}

impl std::fmt::Debug for CustomFiftValue {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt_dump(f)
    }
}

impl tycho_vm::StackValue for CustomFiftValue {
    #[inline]
    fn rc_into_dyn(self: Rc<Self>) -> Rc<dyn tycho_vm::StackValue> {
        self
    }

    fn raw_ty(&self) -> u8 {
        match self.0.ty() {
            StackValueType::String => 100,
            StackValueType::Bytes => 101,
            StackValueType::Cont => 102,
            StackValueType::WordList => 103,
            StackValueType::SharedBox => 104,
            StackValueType::Atom => 105,
            StackValueType::HashMap => 106,
            _ => unreachable!(),
        }
    }

    fn store_as_stack_value(
        &self,
        _: &mut CellBuilder,
        _: &dyn CellContext,
    ) -> Result<(), tycho_types::error::Error> {
        Err(tycho_types::error::Error::InvalidData)
    }

    fn fmt_dump(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt_dump(f)
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
