use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::OnceLock;

use anyhow::Result;
use everscale_types::prelude::CellSlice;

use crate::core::*;

pub struct VmUtils;

#[fift_module]
impl VmUtils {
    #[init]
    fn init(&self, d: &mut Dictionary) -> Result<()> {
        thread_local! {
            static VM_LIBRARIES: Rc<dyn StackValue> = Rc::new(SharedBox::default());
        }

        let vm_libraries = VM_LIBRARIES.with(|b| b.clone());

        d.define_word("vmlibs ", Rc::new(cont::LitCont(vm_libraries)))?;

        Ok(())
    }

    #[cmd(name = "runvmx")]
    #[cmd(name = "dbrunvm")]
    #[cmd(name = "dbrunvm-parallel")]
    #[cmd(name = "vmcont")]
    #[cmd(name = "vmcont@")]
    #[cmd(name = "(vmoplen)")]
    #[cmd(name = "(vmopdump)")]
    fn interpret_run_vm(_ctx: &mut Context) -> Result<()> {
        anyhow::bail!("Unimplemented");
    }
}

fn cp0() -> &'static OpcodeTable {
    fn make_cp0() -> Result<OpcodeTable> {
        let mut t = OpcodeTable::default();
        register_stack_ops(&mut t)?;
        register_tuple_ops(&mut t)?;
        Ok(t)
    }

    static OPCODES: OnceLock<OpcodeTable> = OnceLock::new();
    OPCODES.get_or_init(|| make_cp0().unwrap())
}

fn register_stack_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_simple(0x00, 8, "NOP")?;
    t.add_simple(0x01, 8, "SWAP")?;
    t.add_fixed_range(0x02, 0x10, 4, 4, dump_1sr("XCHG "))?;
    t.add_fixed(0x10, 8, 8, Box::new(dump_xchg))?;
    t.add_fixed(0x11, 8, 8, dump_1sr_l("XCHG "))?;
    t.add_fixed_range(0x12, 0x20, 4, 4, dump_1sr("XCHG s1,"))?;
    t.add_simple(0x20, 8, "DUP")?;
    t.add_simple(0x21, 8, "OVER")?;
    t.add_fixed_range(0x22, 0x30, 4, 4, dump_1sr("PUSH "))?;
    t.add_simple(0x30, 8, "DROP")?;
    t.add_simple(0x31, 8, "NIP")?;
    t.add_fixed_range(0x32, 0x40, 4, 4, dump_1sr("POP "))?;
    t.add_fixed(0x4, 4, 12, dump_3sr("XCHG3 "))?;
    t.add_fixed(0x50, 8, 8, dump_2sr("XCHG2 "))?;
    t.add_fixed(0x51, 8, 8, dump_2sr("XCPU "))?;
    t.add_fixed(0x52, 8, 8, dump_2sr_adj(1, "PUXC "))?;
    t.add_fixed(0x53, 8, 8, dump_2sr("PUSH2 "))?;
    t.add_fixed(0x540, 12, 12, dump_3sr("XCHG3 "))?;
    t.add_fixed(0x541, 12, 12, dump_3sr("XC2PU "))?;
    t.add_fixed(0x542, 12, 12, dump_3sr_adj(1, "XCPUXC "))?;
    t.add_fixed(0x543, 12, 12, dump_3sr("XCPU2 "))?;
    t.add_fixed(0x544, 12, 12, dump_3sr_adj(0x11, "PUXC2 "))?;
    t.add_fixed(0x545, 12, 12, dump_3sr_adj(0x11, "PUXCPU "))?;
    t.add_fixed(0x546, 12, 12, dump_3sr_adj(0x12, "PU2XC "))?;
    t.add_fixed(0x547, 12, 12, dump_3sr("PUSH3 "))?;
    t.add_fixed(0x55, 8, 8, dump_2c_add(0x11, "BLKSWAP ", ","))?;
    t.add_fixed(0x56, 8, 8, dump_1sr_l("PUSH "))?;
    t.add_fixed(0x57, 8, 8, dump_1sr_l("POP "))?;
    t.add_simple(0x58, 8, "ROT")?;
    t.add_simple(0x59, 8, "ROTREV")?;
    t.add_simple(0x5a, 8, "2SWAP")?;
    t.add_simple(0x5b, 8, "2DROP")?;
    t.add_simple(0x5c, 8, "2DUP")?;
    t.add_simple(0x5d, 8, "2OVER")?;
    t.add_fixed(0x5e, 8, 8, dump_2c_add(0x20, "REVERSE ", ","))?;
    t.add_fixed(0x5f0, 12, 4, dump_1c("BLKDROP "))?;
    t.add_fixed_range(0x5f10, 0x6000, 8, 8, dump_2c("BLKPUSH ", ","))?;
    t.add_simple(0x60, 8, "PICK")?;
    t.add_simple(0x61, 8, "ROLL")?;
    t.add_simple(0x62, 8, "ROLLREV")?;
    t.add_simple(0x63, 8, "BLKSWX")?;
    t.add_simple(0x64, 8, "REVX")?;
    t.add_simple(0x65, 8, "DROPX")?;
    t.add_simple(0x66, 8, "TUCK")?;
    t.add_simple(0x67, 8, "XCHGX")?;
    t.add_simple(0x68, 8, "DEPTH")?;
    t.add_simple(0x69, 8, "CHKDEPTH")?;
    t.add_simple(0x6a, 8, "ONLYTOPX")?;
    t.add_simple(0x6b, 8, "ONLYX")?;
    t.add_fixed_range(0x6c10, 0x6d00, 8, 8, dump_2c("BLKDROP2 ", ","))?;
    Ok(())
}

fn register_tuple_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_simple(0x6d, 8, "PUSHNULL")?;
    t.add_simple(0x6e, 8, "ISNULL")?;
    t.add_fixed(0x6f0, 12, 4, dump_1c("TUPLE "))?;
    t.add_fixed(0x6f1, 12, 4, dump_1c("INDEX "))?;
    t.add_fixed(0x6f2, 12, 4, dump_1c("UNTUPLE "))?;
    t.add_fixed(0x6f3, 12, 4, dump_1c("UNPACKFIRST "))?;
    t.add_fixed(0x6f4, 12, 4, dump_1c("EXPLODE "))?;
    t.add_fixed(0x6f5, 12, 4, dump_1c("SETINDEX "))?;
    t.add_fixed(0x6f6, 12, 4, dump_1c("INDEXQ "))?;
    t.add_fixed(0x6f7, 12, 4, dump_1c("SETINDEXQ "))?;
    t.add_simple(0x6f80, 16, "TUPLEVAR")?;
    t.add_simple(0x6f81, 16, "INDEXVAR")?;
    t.add_simple(0x6f82, 16, "UNTUPLEVAR")?;
    t.add_simple(0x6f83, 16, "UNPACKFIRSTVAR")?;
    t.add_simple(0x6f84, 16, "EXPLODEVAR")?;
    t.add_simple(0x6f85, 16, "SETINDEXVAR")?;
    t.add_simple(0x6f86, 16, "INDEXVARQ")?;
    t.add_simple(0x6f87, 16, "SETINDEXVARQ")?;
    t.add_simple(0x6f88, 16, "TLEN")?;
    t.add_simple(0x6f89, 16, "QTLEN")?;
    t.add_simple(0x6f8a, 16, "ISTUPLE")?;
    t.add_simple(0x6f8b, 16, "LAST")?;
    t.add_simple(0x6f8c, 16, "TPUSH")?;
    t.add_simple(0x6f8d, 16, "TPOP")?;

    t.add_simple(0x6fa0, 16, "NULLSWAPIF")?;
    t.add_simple(0x6fa1, 16, "NULLSWAPIFNOT")?;
    t.add_simple(0x6fa2, 16, "NULLROTRIF")?;
    t.add_simple(0x6fa3, 16, "NULLROTRIFNOT")?;
    t.add_simple(0x6fa4, 16, "NULLSWAPIF2")?;
    t.add_simple(0x6fa5, 16, "NULLSWAPIFNOT2")?;
    t.add_simple(0x6fa6, 16, "NULLROTRIF2")?;
    t.add_simple(0x6fa7, 16, "NULLROTRIFNOT2")?;

    t.add_fixed(0x6fb, 12, 4, Box::new(dump_tuple_index2))?;
    t.add_fixed(0x6bfc >> 2, 10, 6, Box::new(dump_tuple_index3))?;
    Ok(())
}

fn register_arith_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_fixed(0x7, 4, 4, Box::new(dump_push_tinyint4))?;
    t.add_fixed(0x80, 8, 8, dump_arg_prefix("PUSHINT "))?;
    t.add_fixed(0x81, 8, 16, dump_arg_prefix("PUSHINT "))?;
    // TODO push int
    t.add_fixed_range(0x8300, 0x83ff, 8, 8, dump_1c_l_add(1, "PUSHPOW2 "))?;
    t.add_simple(0x83ff, 16, "PUSHNAN")?;
    t.add_fixed(0x84, 8, 8, dump_1c_l_add(1, "PUSHPOW2DEC "))?;
    t.add_fixed(0x85, 8, 8, dump_1c_l_add(1, "PUSHNEGPOW2 "))?;

    t.add_simple(0xa0, 8, "ADD")?;
    t.add_simple(0xa1, 8, "SUB")?;
    t.add_simple(0xa2, 8, "SUBR")?;
    t.add_simple(0xa3, 8, "NEGATE")?;
    t.add_simple(0xa4, 8, "INC")?;
    t.add_simple(0xa5, 8, "DEC")?;
    t.add_fixed(0xa6, 8, 8, dump_arg_prefix("ADDINT "))?;
    t.add_fixed(0xa7, 8, 8, dump_arg_prefix("MULINT "))?;
    t.add_simple(0xa8, 8, "MUL")?;

    t.add_simple(0xb7a0, 16, "QADD")?;
    t.add_simple(0xb7a1, 16, "QSUB")?;
    t.add_simple(0xb7a2, 16, "QSUBR")?;
    t.add_simple(0xb7a3, 16, "QNEGATE")?;
    t.add_simple(0xb7a4, 16, "QINC")?;
    t.add_simple(0xb7a5, 16, "QDEC")?;
    t.add_fixed(0xb7a6, 16, 8, dump_arg_prefix("QADDINT "))?;
    t.add_fixed(0xb7a7, 16, 8, dump_arg_prefix("QMULINT "))?;
    t.add_simple(0xb7a8, 16, "QMUL")?;

    t.add_fixed(0xa90, 12, 4, dump_divmod(false))?;

    Ok(())
}

fn dump_xchg(_: &CellSlice<'_>, args: u32, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let x = (args >> 4) & 0xf;
    let y = args & 0xf;
    if x != 0 && x < y {
        write!(f, "XCHG s{x},s{y}")
    } else {
        Ok(())
    }
}

fn dump_tuple_index2(
    _: &CellSlice<'_>,
    args: u32,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    let i = (args >> 2) & 0b11;
    let j = args & 0b11;
    write!(f, "INDEX2 {i},{j}")
}

fn dump_tuple_index3(
    _: &CellSlice<'_>,
    args: u32,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    let i = (args >> 4) & 0b11;
    let j = (args >> 2) & 0b11;
    let k = args & 0b11;
    write!(f, "INDEX3 {i},{j},{k}")
}

fn dump_push_tinyint4(
    _: &CellSlice<'_>,
    args: u32,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    let x = ((args + 5) & 0xf) - 5;
    write!(f, "PUSHINT {x}")
}

fn dump_arg_prefix(op: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, x, f| write!(f, "{op}{x}"))
}

fn dump_divmod(quiet: bool) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        let round_mode = args & 0b11;
        if args & 0b1100 != 0 || round_mode < 3 {
            if quiet {
                f.write_str("Q")?;
            }
            if args & 0b0100 != 0 {
                f.write_str("DIV")?;
            }
            if args & 0b1000 != 0 {
                f.write_str("MOD")?;
            }
            f.write_str(match round_mode {
                1 => "R",
                2 => "C",
                _ => "",
            })?;
        }
        Ok(())
    })
}

#[derive(Default)]
struct OpcodeTable {
    opcodes: BTreeMap<u32, Box<dyn Opcode>>,
}

impl OpcodeTable {
    fn add_simple(&mut self, opcode: u32, bits: u16, name: &str) -> Result<()> {
        todo!()
    }

    fn add_fixed(
        &mut self,
        opcode: u32,
        opcode_bits: u16,
        arg_bits: u16,
        dump: Box<FnDumpArgInstr>,
    ) -> Result<()> {
        todo!()
    }

    fn add_fixed_range(
        &mut self,
        opcode_min: u32,
        opcode_max: u32,
        opcode_bits: u16,
        arg_bits: u16,
        dump: Box<FnDumpArgInstr>,
    ) -> Result<()> {
        todo!()
    }
}

trait Opcode: Send + Sync {
    fn range(&self) -> (u32, u32);

    fn compute_opcode_len(&self, slice: &CellSlice<'_>, opcode: u32, bits: u16) -> Option<u16>;

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        opcode: u32,
        bits: u16,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result;
}

struct SimpleOpcode {
    name: &'static str,
    opcode: u32,
    opcode_bits: u16,
}

impl Opcode for SimpleOpcode {
    fn range(&self) -> (u32, u32) {
        (self.opcode, self.opcode)
    }

    fn compute_opcode_len(&self, _: &CellSlice<'_>, _: u32, bits: u16) -> Option<u16> {
        if bits < self.opcode_bits {
            None
        } else {
            Some(self.opcode_bits)
        }
    }

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        _: u32,
        bits: u16,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        if bits >= self.opcode_bits {
            slice.try_advance(self.opcode_bits, 0);
            f.write_str(self.name)?;
        }
        Ok(())
    }
}

struct FixedOpcode {
    dump: Box<FnDumpArgInstr>,
    opcode_min: u32,
    opcode_max: u32,
    opcode_bits: u16,
    total_bits: u16,
}

impl Opcode for FixedOpcode {
    fn range(&self) -> (u32, u32) {
        (self.opcode_min, self.opcode_max)
    }

    fn compute_opcode_len(&self, _: &CellSlice<'_>, _: u32, bits: u16) -> Option<u16> {
        if bits < self.total_bits {
            None
        } else {
            Some(self.total_bits)
        }
    }

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        opcode: u32,
        bits: u16,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        if bits >= self.total_bits as _ {
            slice.try_advance(self.total_bits, 0);
            (self.dump)(&slice, opcode >> (self.opcode_bits - self.total_bits), f)?;
        }
        Ok(())
    }
}

fn dump_1sr(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| write!(f, "{prefix}s{}", args & 0xf))
}

fn dump_1sr_l(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| write!(f, "{prefix}s{}", args & 0xff))
}

fn dump_1c(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| write!(f, "{prefix}{}", args & 0xf))
}

fn dump_1c_l_add(add: u32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| write!(f, "{prefix}{}", (args & 0xff) + add))
}

fn dump_2sr(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| write!(f, "{prefix}s{},s{}", (args >> 4) & 0xf, args & 0xf))
}

fn dump_2sr_adj(adj: u32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}s{},s{}",
            ((args >> 4) & 0xf) - ((adj >> 4) & 0xf),
            (args & 0xf) - (adj & 0xf)
        )
    })
}

fn dump_2c(prefix: &'static str, sep: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| write!(f, "{prefix}{}{sep}{}", (args >> 4) & 0xf, args & 0xf))
}

fn dump_2c_add(add: u32, prefix: &'static str, sep: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}{}{sep}{}",
            ((args >> 4) & 0xf) + ((add >> 4) & 0xf),
            (args & 0xf) + (add & 0xf)
        )
    })
}

fn dump_3sr(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}s{},s{},s{}",
            (args >> 8) & 0xf,
            (args >> 4) & 0xf,
            args & 0xf
        )
    })
}

fn dump_3sr_adj(adj: u32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}s{},s{},s{}",
            ((args >> 8) & 0xf) - ((adj >> 8) & 0xf),
            ((args >> 4) & 0xf) - ((adj >> 4) & 0xf),
            (args & 0xf) - (adj & 0xf)
        )
    })
}

type FnDumpArgInstr = dyn Fn(&CellSlice<'_>, u32, &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    + Send
    + Sync
    + 'static;
