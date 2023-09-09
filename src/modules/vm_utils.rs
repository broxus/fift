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
    fn interpret_run_vm(_ctx: &mut Context) -> Result<()> {
        anyhow::bail!("Unimplemented");
    }

    #[cmd(name = "(vmoplen)", stack)]
    fn interpret_vmop_len(stack: &mut Stack) -> Result<()> {
        let cp = stack.pop_smallint_signed_range(i32::MIN, i32::MAX)?;
        anyhow::ensure!(cp == 0, "Unknown VM codepage");

        let cs_raw = stack.pop_slice()?;
        let cs = cs_raw.apply()?;

        let (bits, refs) = cp0().compute_len(&cs).unwrap_or_default();
        stack.push_int(((refs as u64) << 16) | (bits as u64))
    }

    #[cmd(name = "(vmopdump)", stack)]
    fn interpret_vmopdump(stack: &mut Stack) -> Result<()> {
        let cp = stack.pop_smallint_signed_range(i32::MIN, i32::MAX)?;
        anyhow::ensure!(cp == 0, "Unknown VM codepage");

        let mut cs_raw = stack.pop_slice()?;
        let mut cs = cs_raw.apply()?;

        let mut dump = String::new();
        cp0().load_dump(&mut cs, &mut dump)?;

        let range = cs.range();
        Rc::make_mut(&mut cs_raw).set_range(range);

        stack.push_raw(cs_raw)?;
        stack.push(dump)
    }
}

fn cp0() -> &'static DispatchTable {
    fn make_cp0() -> Result<DispatchTable> {
        let mut t = OpcodeTable::default();
        register_stack_ops(&mut t)?;
        register_tuple_ops(&mut t)?;
        register_arith_ops(&mut t)?;
        register_cell_ops(&mut t)?;
        register_ton_ops(&mut t)?;
        register_codepage_ops(&mut t)?;
        Ok(t.finalize())
    }

    static OPCODES: OnceLock<DispatchTable> = OnceLock::new();
    OPCODES.get_or_init(|| make_cp0().unwrap())
}

fn register_stack_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_simple(0x00, 8, "NOP")?;
    t.add_simple(0x01, 8, "SWAP")?;
    t.add_fixed_range(0x02, 0x10, 8, 4, dump_1sr("XCHG "))?;
    t.add_fixed(0x10, 8, 8, Box::new(dump_xchg))?;
    t.add_fixed(0x11, 8, 8, dump_1sr_l("XCHG "))?;
    t.add_fixed_range(0x12, 0x20, 8, 4, dump_1sr("XCHG s1,"))?;
    t.add_simple(0x20, 8, "DUP")?;
    t.add_simple(0x21, 8, "OVER")?;
    t.add_fixed_range(0x22, 0x30, 8, 4, dump_1sr("PUSH "))?;
    t.add_simple(0x30, 8, "DROP")?;
    t.add_simple(0x31, 8, "NIP")?;
    t.add_fixed_range(0x32, 0x40, 8, 4, dump_1sr("POP "))?;
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
    t.add_fixed_range(0x5f10, 0x6000, 16, 8, dump_2c("BLKPUSH ", ","))?;
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
    t.add_fixed_range(0x6c10, 0x6d00, 16, 8, dump_2c("BLKDROP2 ", ","))?;
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
    t.add_fixed(0x6fc >> 2, 10, 6, Box::new(dump_tuple_index3))?;
    Ok(())
}

fn register_arith_ops(t: &mut OpcodeTable) -> Result<()> {
    // Int const:
    t.add_fixed(0x7, 4, 4, Box::new(dump_push_tinyint4))?;
    t.add_fixed(0x80, 8, 8, dump_arg_prefix("PUSHINT "))?;
    t.add_fixed(0x81, 8, 16, dump_arg_prefix("PUSHINT "))?;
    t.add_ext_range(
        0x82 << 5,
        (0x82 << 5) + 31,
        13,
        5,
        Box::new(dump_push_int),
        Box::new(compute_len_push_int),
    )?;
    t.add_fixed_range(0x8300, 0x83ff, 16, 8, dump_1c_l_add(1, "PUSHPOW2 "))?;
    t.add_simple(0x83ff, 16, "PUSHNAN")?;
    t.add_fixed(0x84, 8, 8, dump_1c_l_add(1, "PUSHPOW2DEC "))?;
    t.add_fixed(0x85, 8, 8, dump_1c_l_add(1, "PUSHNEGPOW2 "))?;

    // Add/mul
    t.add_simple(0xa0, 8, "ADD")?;
    t.add_simple(0xa1, 8, "SUB")?;
    t.add_simple(0xa2, 8, "SUBR")?;
    t.add_simple(0xa3, 8, "NEGATE")?;
    t.add_simple(0xa4, 8, "INC")?;
    t.add_simple(0xa5, 8, "DEC")?;
    t.add_fixed(0xa6, 8, 8, dump_arg_prefix("ADDINT "))?;
    t.add_fixed(0xa7, 8, 8, dump_arg_prefix("MULINT "))?;
    t.add_simple(0xa8, 8, "MUL")?;

    // Quiet add/mul
    t.add_simple(0xb7a0, 16, "QADD")?;
    t.add_simple(0xb7a1, 16, "QSUB")?;
    t.add_simple(0xb7a2, 16, "QSUBR")?;
    t.add_simple(0xb7a3, 16, "QNEGATE")?;
    t.add_simple(0xb7a4, 16, "QINC")?;
    t.add_simple(0xb7a5, 16, "QDEC")?;
    t.add_fixed(0xb7a6, 16, 8, dump_arg_prefix("QADDINT "))?;
    t.add_fixed(0xb7a7, 16, 8, dump_arg_prefix("QMULINT "))?;
    t.add_simple(0xb7a8, 16, "QMUL")?;

    // Div
    t.add_fixed(0xa90, 12, 4, dump_divmod(false))?;
    t.add_fixed(0xa92, 12, 4, dump_shrmod(false, false))?;
    t.add_fixed(0xa93, 12, 12, dump_shrmod(true, false))?;

    t.add_fixed(0xa98, 12, 4, dump_muldivmod(false))?;

    t.add_fixed(0xa9a, 12, 4, dump_mulshrmod(false, false))?;
    t.add_fixed(0xa9b, 12, 12, dump_mulshrmod(true, false))?;
    t.add_fixed(0xa9c, 12, 4, dump_shldivmod(false, false))?;
    t.add_fixed(0xa9d, 12, 12, dump_shldivmod(true, false))?;

    // Quiet div
    t.add_fixed(0xb7a90, 20, 4, dump_divmod(true))?;
    t.add_fixed(0xb7a92, 20, 4, dump_muldivmod(true))?;
    t.add_fixed(0xb7a9a, 20, 4, dump_mulshrmod(false, true))?;
    t.add_fixed(0xb7a9c, 20, 4, dump_shldivmod(false, true))?;

    // Shift/logic
    t.add_fixed(0xaa, 8, 8, dump_1c_l_add(1, "LSHIFT "))?;
    t.add_fixed(0xab, 8, 8, dump_1c_l_add(1, "RSHIFT "))?;
    t.add_simple(0xac, 8, "LSHIFT")?;
    t.add_simple(0xad, 8, "RSHIFT")?;
    t.add_simple(0xae, 8, "POW2")?;

    t.add_simple(0xb0, 8, "AND")?;
    t.add_simple(0xb1, 8, "OR")?;
    t.add_simple(0xb2, 8, "XOR")?;
    t.add_simple(0xb3, 8, "NOT")?;
    t.add_fixed(0xb4, 8, 8, dump_1c_l_add(1, "FITS "))?;
    t.add_fixed(0xb5, 8, 8, dump_1c_l_add(1, "UFITS "))?;
    t.add_simple(0xb600, 16, "FITSX")?;
    t.add_simple(0xb601, 16, "UFITSX")?;
    t.add_simple(0xb602, 16, "BITSIZE")?;
    t.add_simple(0xb603, 16, "UBITSIZE")?;

    // Quiet shift/logic
    t.add_fixed(0xb7aa, 16, 8, dump_1c_l_add(1, "QLSHIFT "))?;
    t.add_fixed(0xb7ab, 16, 8, dump_1c_l_add(1, "QRSHIFT "))?;
    t.add_simple(0xb7ac, 16, "QLSHIFT")?;
    t.add_simple(0xb7ad, 16, "QRSHIFT")?;
    t.add_simple(0xb7ae, 16, "QPOW2")?;

    t.add_simple(0xb7b0, 16, "QAND")?;
    t.add_simple(0xb7b1, 16, "QOR")?;
    t.add_simple(0xb7b2, 16, "QXOR")?;
    t.add_simple(0xb7b3, 16, "QNOT")?;
    t.add_fixed(0xb7b4, 16, 8, dump_1c_l_add(1, "QFITS "))?;
    t.add_fixed(0xb7b5, 16, 8, dump_1c_l_add(1, "QUFITS "))?;
    t.add_simple(0xb7b600, 24, "QFITSX")?;
    t.add_simple(0xb7b601, 24, "QUFITSX")?;
    t.add_simple(0xb7b602, 24, "QBITSIZE")?;
    t.add_simple(0xb7b603, 24, "QUBITSIZE")?;

    // Other
    t.add_simple(0xb608, 16, "MIN")?;
    t.add_simple(0xb609, 16, "MAX")?;
    t.add_simple(0xb60a, 16, "MINMAX")?;
    t.add_simple(0xb60b, 16, "ABS")?;

    // Quiet other
    t.add_simple(0xb7b608, 24, "QMIN")?;
    t.add_simple(0xb7b609, 24, "QMAX")?;
    t.add_simple(0xb7b60a, 24, "QMINMAX")?;
    t.add_simple(0xb7b60b, 24, "QABS")?;

    // Int cmp
    t.add_simple(0xb8, 8, "SGN")?;
    t.add_simple(0xb9, 8, "LESS")?;
    t.add_simple(0xba, 8, "EQUAL")?;
    t.add_simple(0xbb, 8, "LEQ")?;
    t.add_simple(0xbc, 8, "GREATER")?;
    t.add_simple(0xbd, 8, "NEQ")?;
    t.add_simple(0xbe, 8, "GEQ")?;
    t.add_simple(0xbf, 8, "CMP")?;
    t.add_fixed(0xc0, 8, 8, dump_arg_prefix("EQINT "))?;
    t.add_fixed(0xc1, 8, 8, dump_arg_prefix("LESSINT "))?;
    t.add_fixed(0xc2, 8, 8, dump_arg_prefix("GTINT "))?;
    t.add_fixed(0xc3, 8, 8, dump_arg_prefix("NEQINT "))?;
    t.add_simple(0xc4, 8, "ISNAN")?;
    t.add_simple(0xc5, 8, "CHKNAN")?;

    // Quiet imt cmp
    t.add_simple(0xb7b8, 16, "QSGN")?;
    t.add_simple(0xb7b9, 16, "QLESS")?;
    t.add_simple(0xb7ba, 16, "QEQUAL")?;
    t.add_simple(0xb7bb, 16, "QLEQ")?;
    t.add_simple(0xb7bc, 16, "QGREATER")?;
    t.add_simple(0xb7bd, 16, "QNEQ")?;
    t.add_simple(0xb7be, 16, "QGEQ")?;
    t.add_simple(0xb7bf, 16, "QCMP")?;
    t.add_fixed(0xb7c0, 16, 8, dump_arg_prefix("QEQINT "))?;
    t.add_fixed(0xb7c1, 16, 8, dump_arg_prefix("QLESSINT "))?;
    t.add_fixed(0xb7c2, 16, 8, dump_arg_prefix("QGTINT "))?;
    t.add_fixed(0xb7c3, 16, 8, dump_arg_prefix("QNEQINT "))?;

    Ok(())
}

#[rustfmt::skip]
fn register_cell_ops(t: &mut OpcodeTable) -> Result<()> {
    // Cell const
    t.add_ext(0x88, 8, 0, dump_push_ref("PUSHREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0x89, 8, 0, dump_push_ref("PUSHREFSLICE"), Box::new(compute_len_push_ref))?;
    t.add_ext(0x8a, 8, 0, dump_push_ref("PUSHREFCONT"), Box::new(compute_len_push_ref))?;

    // TODO

    Ok(())
}

fn register_ton_ops(t: &mut OpcodeTable) -> Result<()> {
    // Basic gas
    t.add_simple(0xf800, 16, "ACCEPT")?;
    t.add_simple(0xf801, 16, "SETGASLIMIT")?;
    t.add_simple(0xf80f, 16, "COMMIT")?;

    // PRNG
    t.add_simple(0xf810, 16, "RANDU256")?;
    t.add_simple(0xf811, 16, "RAND")?;
    t.add_simple(0xf814, 16, "SETRAND")?;
    t.add_simple(0xf815, 16, "ADDRAND")?;

    // Config
    t.add_fixed_range(0xf820, 0xf823, 16, 4, dump_1c("GETPARAM "))?;
    t.add_simple(0xf823, 16, "NOW")?;
    t.add_simple(0xf824, 16, "BLOCKLT")?;
    t.add_simple(0xf825, 16, "LTIME")?;
    t.add_simple(0xf826, 16, "RANDSEED")?;
    t.add_simple(0xf827, 16, "BALANCE")?;
    t.add_simple(0xf828, 16, "MYADDR")?;
    t.add_simple(0xf829, 16, "CONFIGROOT")?;
    t.add_fixed_range(0xf82a, 0xf830, 16, 4, dump_1c("GETPARAM "))?;
    t.add_simple(0xf830, 16, "CONFIGDICT")?;
    t.add_simple(0xf832, 16, "CONFIGPARAM")?;
    t.add_simple(0xf833, 16, "CONFIGOPTPARAM")?;
    t.add_simple(0xf840, 16, "GETGLOBVAR")?;
    t.add_fixed_range(0xf841, 0xf860, 16, 5, dump_1c_and(31, "GETGLOB "))?;
    t.add_simple(0xf860, 16, "SETGLOBVAR")?;
    t.add_fixed_range(0xf861, 0xf880, 16, 5, dump_1c_and(31, "SETGLOB "))?;

    // Crypto
    t.add_simple(0xf900, 16, "HASHCU")?;
    t.add_simple(0xf901, 16, "HASHSU")?;
    t.add_simple(0xf902, 16, "SHA256U")?;
    t.add_simple(0xf910, 16, "CHKSIGNU")?;
    t.add_simple(0xf911, 16, "CHKSIGNS")?;

    // Misc
    t.add_simple(0xf940, 16, "CDATASIZEQ")?;
    t.add_simple(0xf941, 16, "CDATASIZE")?;
    t.add_simple(0xf942, 16, "SDATASIZEQ")?;
    t.add_simple(0xf943, 16, "SDATASIZE")?;

    // Currency/address
    t.add_simple(0xfa00, 16, "LDGRAMS")?;
    t.add_simple(0xfa01, 16, "LDVARINT16")?;
    t.add_simple(0xfa02, 16, "STGRAMS")?;
    t.add_simple(0xfa03, 16, "STVARINT16")?;
    t.add_simple(0xfa04, 16, "LDVARUINT32")?;
    t.add_simple(0xfa05, 16, "LDVARINT32")?;
    t.add_simple(0xfa06, 16, "STVARUINT32")?;
    t.add_simple(0xfa07, 16, "STVARINT32")?;

    t.add_simple(0xfa40, 16, "LDMSGADDR")?;
    t.add_simple(0xfa41, 16, "LDMSGADDRQ")?;
    t.add_simple(0xfa42, 16, "PARSEMSGADDR")?;
    t.add_simple(0xfa43, 16, "PARSEMSGADDRQ")?;
    t.add_simple(0xfa44, 16, "REWRITESTDADDR")?;
    t.add_simple(0xfa45, 16, "REWRITESTDADDRQ")?;
    t.add_simple(0xfa46, 16, "REWRITEVARADDR")?;
    t.add_simple(0xfa47, 16, "REWRITEVARADDRQ")?;

    // Message
    t.add_simple(0xfb00, 16, "SENDRAWMSG")?;
    t.add_simple(0xfb02, 16, "RAWRESERVE")?;
    t.add_simple(0xfb03, 16, "RAWRESERVEX")?;
    t.add_simple(0xfb04, 16, "SETCODE")?;
    t.add_simple(0xfb06, 16, "SETLIBCODE")?;
    t.add_simple(0xfb07, 16, "CHANGELIB")?;

    Ok(())
}

fn register_codepage_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_fixed_range(0xff00, 0xfff0, 16, 8, dump_1c_and(0xff, "SETCP "))?;
    t.add_fixed_range(0xfff1, 0x10000, 16, 8, dump_1c_l_add(-256, "SETCP "))?;
    t.add_simple(0xfff0, 16, "SETCPX")?;

    Ok(())
}

struct DispatchTable {
    opcodes: Vec<(u32, Box<dyn Opcode>)>,
}

impl DispatchTable {
    fn get_opcode_from_slice(slice: &CellSlice<'_>) -> (u32, u16) {
        let bits = std::cmp::min(MAX_OPCODE_BITS, slice.remaining_bits());
        let opcode = (slice.get_uint(0, bits).unwrap() as u32) << (MAX_OPCODE_BITS - bits);
        (opcode, bits)
    }

    fn load_dump(&self, slice: &mut CellSlice<'_>, f: &mut dyn std::fmt::Write) -> Result<()> {
        let (opcode, bits) = Self::get_opcode_from_slice(slice);
        let op = self.lookup(opcode);
        op.load_dump(slice, opcode, bits, f)
    }

    fn compute_len(&self, slice: &CellSlice<'_>) -> Option<(u16, u8)> {
        let (opcode, bits) = Self::get_opcode_from_slice(slice);
        let op = self.lookup(opcode);
        op.compute_len(slice, opcode, bits)
    }

    fn lookup(&self, opcode: u32) -> &dyn Opcode {
        let mut i = 0;
        let mut j = self.opcodes.len();
        while j - i > 1 {
            let k = (j + i) >> 1;
            if self.opcodes[k].0 <= opcode {
                i = k;
            } else {
                j = k;
            }
        }
        self.opcodes[i].1.as_ref()
    }
}

#[derive(Default)]
struct OpcodeTable {
    opcodes: BTreeMap<u32, Box<dyn Opcode>>,
}

impl OpcodeTable {
    fn finalize(self) -> DispatchTable {
        let mut opcodes = Vec::with_capacity(self.opcodes.len() * 2 + 1);

        let mut upto = 0;
        for (k, opcode) in self.opcodes {
            let (min, max) = opcode.range();
            if min > upto {
                opcodes.push((
                    upto,
                    Box::new(DummyOpcode {
                        opcode_min: upto,
                        opcode_max: min,
                    }) as Box<_>,
                ));
            }

            opcodes.push((k, opcode));
            upto = max;
        }

        if upto < MAX_OPCODE {
            opcodes.push((
                upto,
                Box::new(DummyOpcode {
                    opcode_min: upto,
                    opcode_max: MAX_OPCODE,
                }),
            ));
        }

        opcodes.shrink_to_fit();

        DispatchTable { opcodes }
    }

    fn add_simple(&mut self, opcode: u32, bits: u16, name: &'static str) -> Result<()> {
        let remaining_bits = MAX_OPCODE_BITS - bits;
        self.add_opcode(Box::new(SimpleOpcode {
            name,
            opcode_min: opcode << remaining_bits,
            opcode_max: (opcode + 1) << remaining_bits,
            bits,
        }))
    }

    fn add_fixed(
        &mut self,
        opcode: u32,
        opcode_bits: u16,
        arg_bits: u16,
        dump: Box<FnDumpArgInstr>,
    ) -> Result<()> {
        let remaining_bits = MAX_OPCODE_BITS - opcode_bits;
        self.add_opcode(Box::new(FixedOpcode {
            dump,
            opcode_min: opcode << remaining_bits,
            opcode_max: (opcode + 1) << remaining_bits,
            // opcode_bits,
            total_bits: opcode_bits + arg_bits,
        }))
    }

    fn add_fixed_range(
        &mut self,
        opcode_min: u32,
        opcode_max: u32,
        total_bits: u16,
        _arg_bits: u16,
        dump: Box<FnDumpArgInstr>,
    ) -> Result<()> {
        let remaining_bits = MAX_OPCODE_BITS - total_bits;
        self.add_opcode(Box::new(FixedOpcode {
            dump,
            opcode_min: opcode_min << remaining_bits,
            opcode_max: opcode_max << remaining_bits,
            // opcode_bits: total_bits - arg_bits,
            total_bits,
        }))
    }

    fn add_ext(
        &mut self,
        opcode: u32,
        opcode_bits: u16,
        arg_bits: u16,
        dump: Box<FnDumpInstr>,
        instr_len: Box<FnComputeInstrLen>,
    ) -> Result<()> {
        let remaining_bits = MAX_OPCODE_BITS - opcode_bits;
        self.add_opcode(Box::new(ExtOpcode {
            dump,
            instr_len,
            opcode_min: opcode << remaining_bits,
            opcode_max: (opcode + 1) << remaining_bits,
            total_bits: opcode_bits + arg_bits,
        }))
    }

    fn add_ext_range(
        &mut self,
        opcode_min: u32,
        opcode_max: u32,
        total_bits: u16,
        _arg_bits: u16,
        dump: Box<FnDumpInstr>,
        instr_len: Box<FnComputeInstrLen>,
    ) -> Result<()> {
        let remaining_bits = MAX_OPCODE_BITS - total_bits;
        self.add_opcode(Box::new(ExtOpcode {
            dump,
            instr_len,
            opcode_min: opcode_min << remaining_bits,
            opcode_max: opcode_max << remaining_bits,
            total_bits,
        }))
    }

    fn add_opcode(&mut self, opcode: Box<dyn Opcode>) -> Result<()> {
        let (min, max) = opcode.range();
        debug_assert!(min < max);
        debug_assert!(max <= MAX_OPCODE);

        if let Some((other_min, _)) = self.opcodes.range(min..).next() {
            anyhow::ensure!(
                max <= *other_min,
                "Opcode overlaps with next min: {other_min:06x}"
            );
        }

        if let Some((k, prev)) = self.opcodes.range(..=min).next_back() {
            let (prev_min, prev_max) = prev.range();
            debug_assert!(prev_min < prev_max);
            debug_assert!(prev_min == *k);
            anyhow::ensure!(
                prev_max <= min,
                "Opcode overlaps with prev max: {prev_max:06x}"
            );
        }

        self.opcodes.insert(min, opcode);
        Ok(())
    }
}

const MAX_OPCODE_BITS: u16 = 24;
const MAX_OPCODE: u32 = 1 << MAX_OPCODE_BITS;

trait Opcode: Send + Sync {
    fn range(&self) -> (u32, u32);

    fn compute_len(&self, slice: &CellSlice<'_>, opcode: u32, bits: u16) -> Option<(u16, u8)>;

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        opcode: u32,
        bits: u16,
        f: &mut dyn std::fmt::Write,
    ) -> Result<()>;
}

struct DummyOpcode {
    opcode_min: u32,
    opcode_max: u32,
}

impl Opcode for DummyOpcode {
    fn range(&self) -> (u32, u32) {
        (self.opcode_min, self.opcode_max)
    }

    fn compute_len(&self, _: &CellSlice<'_>, _: u32, _: u16) -> Option<(u16, u8)> {
        None
    }

    fn load_dump(
        &self,
        _: &mut CellSlice<'_>,
        _: u32,
        _: u16,
        _: &mut dyn std::fmt::Write,
    ) -> Result<()> {
        Ok(())
    }
}

struct SimpleOpcode {
    name: &'static str,
    opcode_min: u32,
    opcode_max: u32,
    bits: u16,
}

impl Opcode for SimpleOpcode {
    fn range(&self) -> (u32, u32) {
        (self.opcode_min, self.opcode_max)
    }

    fn compute_len(&self, _: &CellSlice<'_>, _: u32, bits: u16) -> Option<(u16, u8)> {
        (bits >= self.bits).then_some((self.bits, 0))
    }

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        _: u32,
        bits: u16,
        f: &mut dyn std::fmt::Write,
    ) -> Result<()> {
        if bits >= self.bits {
            slice.try_advance(self.bits, 0);
            f.write_str(self.name)?;
        }
        Ok(())
    }
}

struct FixedOpcode {
    dump: Box<FnDumpArgInstr>,
    opcode_min: u32,
    opcode_max: u32,
    total_bits: u16,
}

impl Opcode for FixedOpcode {
    fn range(&self) -> (u32, u32) {
        (self.opcode_min, self.opcode_max)
    }

    fn compute_len(&self, _: &CellSlice<'_>, _: u32, bits: u16) -> Option<(u16, u8)> {
        (bits >= self.total_bits).then_some((self.total_bits, 0))
    }

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        opcode: u32,
        bits: u16,
        f: &mut dyn std::fmt::Write,
    ) -> Result<()> {
        if bits >= self.total_bits {
            slice.try_advance(self.total_bits, 0);
            (self.dump)(slice, opcode >> (MAX_OPCODE_BITS - self.total_bits), f)?;
        }
        Ok(())
    }
}

struct ExtOpcode {
    dump: Box<FnDumpInstr>,
    instr_len: Box<FnComputeInstrLen>,
    opcode_min: u32,
    opcode_max: u32,
    total_bits: u16,
}

impl Opcode for ExtOpcode {
    fn range(&self) -> (u32, u32) {
        (self.opcode_min, self.opcode_max)
    }

    fn compute_len(&self, slice: &CellSlice<'_>, opcode: u32, bits: u16) -> Option<(u16, u8)> {
        if bits >= self.total_bits {
            Some((self.instr_len)(slice, opcode, bits))
        } else {
            None
        }
    }

    fn load_dump(
        &self,
        slice: &mut CellSlice<'_>,
        opcode: u32,
        bits: u16,
        f: &mut dyn std::fmt::Write,
    ) -> Result<()> {
        if bits >= self.total_bits {
            slice.try_advance(self.total_bits, 0);
            (self.dump)(
                slice,
                opcode >> (MAX_OPCODE_BITS - self.total_bits),
                self.total_bits,
                f,
            )?;
        }
        Ok(())
    }
}

type FnDumpArgInstr =
    dyn Fn(&mut CellSlice<'_>, u32, &mut dyn std::fmt::Write) -> Result<()> + Send + Sync + 'static;
type FnDumpInstr = dyn Fn(&mut CellSlice<'_>, u32, u16, &mut dyn std::fmt::Write) -> Result<()>
    + Send
    + Sync
    + 'static;
type FnComputeInstrLen = dyn Fn(&CellSlice<'_>, u32, u16) -> (u16, u8) + Send + Sync + 'static;

fn dump_arg_prefix(op: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, x, f| {
        write!(f, "{op}{x}")?;
        Ok(())
    })
}

fn dump_1sr(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}s{}", args & 0xf)?;
        Ok(())
    })
}

fn dump_1sr_l(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}s{}", args & 0xff)?;
        Ok(())
    })
}

fn dump_1c(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}{}", args & 0xf)?;
        Ok(())
    })
}

fn dump_1c_and(mask: u32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}{}", args & mask)?;
        Ok(())
    })
}

fn dump_1c_l_add(add: i32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}{}", (args & 0xff) as i32 + add)?;
        Ok(())
    })
}

fn dump_2sr(prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}s{},s{}", (args >> 4) & 0xf, args & 0xf)?;
        Ok(())
    })
}

fn dump_2sr_adj(adj: u32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}s{},s{}",
            ((args >> 4) & 0xf) - ((adj >> 4) & 0xf),
            (args & 0xf) - (adj & 0xf)
        )?;
        Ok(())
    })
}

fn dump_2c(prefix: &'static str, sep: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}{}{sep}{}", (args >> 4) & 0xf, args & 0xf)?;
        Ok(())
    })
}

fn dump_2c_add(add: u32, prefix: &'static str, sep: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}{}{sep}{}",
            ((args >> 4) & 0xf) + ((add >> 4) & 0xf),
            (args & 0xf) + (add & 0xf)
        )?;
        Ok(())
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
        )?;
        Ok(())
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
        )?;
        Ok(())
    })
}

fn dump_xchg(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let x = (args >> 4) & 0xf;
    let y = args & 0xf;
    if x != 0 && x < y {
        write!(f, "XCHG s{x},s{y}")?
    }
    Ok(())
}

fn dump_tuple_index2(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let i = (args >> 2) & 0b11;
    let j = args & 0b11;
    write!(f, "INDEX2 {i},{j}")?;
    Ok(())
}

fn dump_tuple_index3(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let i = (args >> 4) & 0b11;
    let j = (args >> 2) & 0b11;
    let k = args & 0b11;
    write!(f, "INDEX3 {i},{j},{k}")?;
    Ok(())
}

fn write_round_mode(mode: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    f.write_str(match mode {
        0b01 => "R",
        0b10 => "C",
        _ => "",
    })?;
    Ok(())
}

fn dump_push_tinyint4(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let x = ((args + 5) & 0xf) - 5;
    write!(f, "PUSHINT {x}")?;
    Ok(())
}

fn dump_push_int(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let l = ((args & 31) + 2) as u16;
    let value_len = 3 + l * 8;
    if !cs.has_remaining(bits + value_len, 0) {
        return Ok(());
    }

    cs.try_advance(bits, 0);

    let mut bytes = [0u8; 33];
    let rem = value_len % 8;
    let mut int = num_bigint::BigUint::from_bytes_be(cs.load_raw(&mut bytes, bits)?);
    if rem != 0 {
        int >>= 8 - rem;
    }
    write!(f, "PUSHINT {int}")?;
    Ok(())
}

fn compute_len_push_int(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let l = ((args & 31) + 2) as u16;
    let bit_len = bits + 3 + l * 8;
    (bit_len * cs.has_remaining(bit_len, 0) as u16, 0)
}

fn dump_push_ref(name: &'static str) -> Box<FnDumpInstr> {
    Box::new(move |cs, _, bits, f| {
        if !cs.has_remaining(0, 1) {
            return Ok(());
        }
        cs.try_advance(bits, 0);
        let cell = cs.load_reference()?;
        write!(f, "{name} ({})", cell.repr_hash())?;
        Ok(())
    })
}

fn compute_len_push_ref(cs: &CellSlice<'_>, _: u32, bits: u16) -> (u16, u8) {
    if cs.has_remaining(0, 1) {
        (bits, 1)
    } else {
        (0, 0)
    }
}

fn dump_divmod(quiet: bool) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        let round_mode = args & 0b11;
        if args & 0b1100 != 0 && round_mode < 3 {
            if quiet {
                f.write_str("Q")?;
            }
            if args & 0b0100 != 0 {
                f.write_str("DIV")?;
            }
            if args & 0b1000 != 0 {
                f.write_str("MOD")?;
            }
            write_round_mode(round_mode, f)?;
        }
        Ok(())
    })
}

fn dump_shrmod(has_y: bool, quiet: bool) -> Box<FnDumpArgInstr> {
    Box::new(move |_, mut args, f| {
        let round_mode = args & 0b11;
        let mut y = 0;
        if has_y {
            y = (args & 0xff) + 1;
            args >>= 8;
        }
        if args & 0b1100 != 0 && round_mode < 3 {
            if quiet {
                f.write_str("Q")?;
            }
            f.write_str(match args & 0b1100 {
                0b0100 => "RSHIFT",
                0b1000 => "MODPOW2",
                _ => "RSHIFTMOD",
            })?;
            write_round_mode(round_mode, f)?;
            if has_y {
                write!(f, " {y}")?;
            }
        }
        Ok(())
    })
}

fn dump_muldivmod(quiet: bool) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        let round_mode = args & 0b11;
        if args & 0b1100 != 0 && round_mode < 3 {
            if quiet {
                f.write_str("Q")?;
            }
            f.write_str(if args & 0b0100 != 0 { "MULDIV" } else { "MUL" })?;
            if args & 0b1000 != 0 {
                f.write_str("MOD")?;
            }
            write_round_mode(round_mode, f)?;
        }
        Ok(())
    })
}

fn dump_mulshrmod(has_y: bool, quiet: bool) -> Box<FnDumpArgInstr> {
    Box::new(move |_, mut args, f| {
        let round_mode = args & 0b11;
        let mut y = 0;
        if has_y {
            y = (args & 0xff) + 1;
            args >>= 8;
        }
        if args & 0b1100 != 0 && round_mode < 3 {
            if quiet {
                f.write_str("Q")?;
            }
            f.write_str(match args & 0b1100 {
                0b0100 => "MULRSHIFT",
                0b1000 => "MULMODPOW2",
                _ => "MULRSHIFTMOD",
            })?;
            write_round_mode(round_mode, f)?;
            if has_y {
                write!(f, " {y}")?;
            }
        }
        Ok(())
    })
}

fn dump_shldivmod(has_y: bool, quiet: bool) -> Box<FnDumpArgInstr> {
    Box::new(move |_, mut args, f| {
        let round_mode = args & 0b11;
        let mut y = 0;
        if has_y {
            y = (args & 0xff) + 1;
            args >>= 8;
        }
        if args & 0b1100 != 0 && round_mode < 3 {
            if quiet {
                f.write_str("Q")?;
            }
            f.write_str(if args & 0b0100 != 0 {
                "LSHIFTDIV"
            } else {
                "LSHIFT"
            })?;
            if args & 0b1000 != 0 {
                f.write_str("MOD")?;
            }
            write_round_mode(round_mode, f)?;
            if has_y {
                write!(f, " {y}")?;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_opcodes_are_valid() {
        assert!(!cp0().opcodes.is_empty());
    }
}
