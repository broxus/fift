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
        register_continuation_ops(&mut t)?;
        register_dictionary_ops(&mut t)?;
        register_ton_ops(&mut t)?;
        register_debug_ops(&mut t)?;
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
    t.add_fixed(0x80, 8, 8, dump_op_tinyint8("PUSHINT "))?;
    t.add_fixed(0x81, 8, 16, Box::new(dump_push_smallint))?;
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
    t.add_fixed(0xa6, 8, 8, dump_op_tinyint8("ADDINT "))?;
    t.add_fixed(0xa7, 8, 8, dump_op_tinyint8("MULINT "))?;
    t.add_simple(0xa8, 8, "MUL")?;

    // Quiet add/mul
    t.add_simple(0xb7a0, 16, "QADD")?;
    t.add_simple(0xb7a1, 16, "QSUB")?;
    t.add_simple(0xb7a2, 16, "QSUBR")?;
    t.add_simple(0xb7a3, 16, "QNEGATE")?;
    t.add_simple(0xb7a4, 16, "QINC")?;
    t.add_simple(0xb7a5, 16, "QDEC")?;
    t.add_fixed(0xb7a6, 16, 8, dump_op_tinyint8("QADDINT "))?;
    t.add_fixed(0xb7a7, 16, 8, dump_op_tinyint8("QMULINT "))?;
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
    t.add_fixed(0xc0, 8, 8, dump_op_tinyint8("EQINT "))?;
    t.add_fixed(0xc1, 8, 8, dump_op_tinyint8("LESSINT "))?;
    t.add_fixed(0xc2, 8, 8, dump_op_tinyint8("GTINT "))?;
    t.add_fixed(0xc3, 8, 8, dump_op_tinyint8("NEQINT "))?;
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
    t.add_fixed(0xb7c0, 16, 8, dump_op_tinyint8("QEQINT "))?;
    t.add_fixed(0xb7c1, 16, 8, dump_op_tinyint8("QLESSINT "))?;
    t.add_fixed(0xb7c2, 16, 8, dump_op_tinyint8("QGTINT "))?;
    t.add_fixed(0xb7c3, 16, 8, dump_op_tinyint8("QNEQINT "))?;

    Ok(())
}

#[rustfmt::skip]
fn register_cell_ops(t: &mut OpcodeTable) -> Result<()> {
    // Cell const
    t.add_ext(0x88, 8, 0, dump_push_ref("PUSHREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0x89, 8, 0, dump_push_ref("PUSHREFSLICE"), Box::new(compute_len_push_ref))?;
    t.add_ext(0x8a, 8, 0, dump_push_ref("PUSHREFCONT"), Box::new(compute_len_push_ref))?;
    t.add_ext(0x8b, 8, 4, Box::new(dump_push_slice), Box::new(compute_len_push_slice))?;
    t.add_ext(0x8c, 8, 7, Box::new(dump_push_slice_r), Box::new(compute_len_push_slice_r))?;
    t.add_ext_range((0x8d * 8) << 7, (0x8d * 8 + 5) << 7, 18, 10, Box::new(dump_push_slice_r2), Box::new(compute_len_push_slice_r2))?;
    t.add_ext(0x8e / 2, 7, 9, Box::new(dump_push_cont), Box::new(compute_len_push_cont))?;
    t.add_ext(0x9, 4, 4, Box::new(dump_push_cont_simple), Box::new(compute_len_push_cont_simple))?;

    // Cell compare
    t.add_simple(0xc700, 16, "SEMPTY")?;
    t.add_simple(0xc701, 16, "SDEMPTY")?;
    t.add_simple(0xc702, 16, "SREMPTY")?;
    t.add_simple(0xc703, 16, "SDFIRST")?;
    t.add_simple(0xc704, 16, "SDLEXCMP")?;
    t.add_simple(0xc705, 16, "SDEQ")?;

    t.add_simple(0xc708, 16, "SDPFX")?;
    t.add_simple(0xc709, 16, "SDPFXREV")?;
    t.add_simple(0xc70a, 16, "SDPPFX")?;
    t.add_simple(0xc70b, 16, "SDPPFXREV")?;
    t.add_simple(0xc70c, 16, "SDSFX")?;
    t.add_simple(0xc70d, 16, "SDSFXREV")?;
    t.add_simple(0xc70e, 16, "SDPSFX")?;
    t.add_simple(0xc70f, 16, "SDPSFXREV")?;
    t.add_simple(0xc710, 16, "SDCNTLEAD0")?;
    t.add_simple(0xc711, 16, "SDCNTLEAD1")?;
    t.add_simple(0xc712, 16, "SDCNTTRAIL0")?;
    t.add_simple(0xc713, 16, "SDCNTTRAIL1")?;

    // Cell serialization
    t.add_simple(0xc8, 8, "NEWC")?;
    t.add_simple(0xc9, 8, "ENDC")?;
    t.add_fixed(0xca, 8, 8, dump_1c_l_add(1, "STI "))?;
    t.add_fixed(0xcb, 8, 8, dump_1c_l_add(1, "STU "))?;
    t.add_simple(0xcc, 8, "STREF")?;
    t.add_simple(0xcd, 8, "ENDCST")?;
    t.add_simple(0xce, 8, "STSLICE")?;
    t.add_fixed(0xcf00 >> 3, 13, 3, Box::new(dump_store_int_var))?;
    t.add_fixed(0xcf08 >> 3, 13, 11, Box::new(dump_store_int_fixed))?;
    t.add_simple(0xcf10, 16, "STREF")?;
    t.add_simple(0xcf11, 16, "STBREF")?;
    t.add_simple(0xcf12, 16, "STSLICE")?;
    t.add_simple(0xcf13, 16, "STB")?;
    t.add_simple(0xcf14, 16, "STREFR")?;
    t.add_simple(0xcf15, 16, "STBREFR")?;
    t.add_simple(0xcf16, 16, "STSLICER")?;
    t.add_simple(0xcf17, 16, "STBR")?;
    t.add_simple(0xcf18, 16, "STREFQ")?;
    t.add_simple(0xcf19, 16, "STBREFQ")?;
    t.add_simple(0xcf1a, 16, "STSLICEQ")?;
    t.add_simple(0xcf1b, 16, "STBQ")?;
    t.add_simple(0xcf1c, 16, "STREFRQ")?;
    t.add_simple(0xcf1d, 16, "STBREFRQ")?;
    t.add_simple(0xcf1e, 16, "STSLICERQ")?;
    t.add_simple(0xcf1f, 16, "STBRQ")?;
    t.add_ext_range(0xcf20, 0xcf22, 16, 1, Box::new(dump_store_cont_ref), Box::new(compute_len_store_const_ref))?;
    t.add_simple(0xcf23, 16, "ENDXC")?;
    t.add_fixed(0xcf28 >> 2, 14, 2, Box::new(dump_store_le_int))?;
    t.add_simple(0xcf30, 16, "BDEPTH")?;
    t.add_simple(0xcf31, 16, "BBITS")?;
    t.add_simple(0xcf32, 16, "BREFS")?;
    t.add_simple(0xcf33, 16, "BBITREFS")?;

    t.add_simple(0xcf35, 16, "BREMBITS")?;
    t.add_simple(0xcf36, 16, "BREMREFS")?;
    t.add_simple(0xcf37, 16, "BREMBITREFS")?;
    t.add_fixed(0xcf38, 16, 8, dump_1c_l_add(1, "BCHKBITS "))?;
    t.add_simple(0xcf39, 16, "BCHKBITS")?;
    t.add_simple(0xcf3a, 16, "BCHKREFS")?;
    t.add_simple(0xcf3b, 16, "BCHKBITREFS")?;
    t.add_fixed(0xcf3c, 16, 8, dump_1c_l_add(1, "BCHKBITSQ "))?;
    t.add_simple(0xcf3d, 16, "BCHKBITSQ")?;
    t.add_simple(0xcf3e, 16, "BCHKREFSQ")?;
    t.add_simple(0xcf3f, 16, "BCHKBITREFSQ")?;
    t.add_simple(0xcf40, 16, "STZEROES")?;
    t.add_simple(0xcf41, 16, "STONES")?;
    t.add_simple(0xcf42, 16, "STSAME")?;
    t.add_ext(0xcf80 >> 7, 9, 5, Box::new(dump_store_const_slice), Box::new(compute_len_store_const_slice))?;

    // Cell deserialization
    t.add_simple(0xd0, 8, "CTOS")?;
    t.add_simple(0xd1, 8, "ENDS")?;
    t.add_fixed(0xd2, 8, 8, dump_1c_l_add(1, "LDI "))?;
    t.add_fixed(0xd3, 8, 8, dump_1c_l_add(1, "LDU "))?;
    t.add_simple(0xd4, 8, "LDREF")?;
    t.add_simple(0xd5, 8, "LDREFRTOS")?;
    t.add_fixed(0xd6, 8, 8, dump_1c_l_add(1, "LDSLICE "))?;
    t.add_fixed(0xd700 >> 3, 13, 3, Box::new(dump_load_int_var))?;
    t.add_fixed(0xd708 >> 3, 13, 11, Box::new(dump_load_int_fixed2))?;
    t.add_fixed(0xd710 >> 3, 13, 3, Box::new(dump_preload_uint_fixed_0e))?;
    t.add_fixed(0xd718 >> 2, 14, 2, Box::new(dump_load_slice))?;
    t.add_fixed(0xd71c >> 2, 14, 10, Box::new(dump_load_slice_fixed2))?;
    t.add_simple(0xd720, 16, "SDCUTFIRST")?;
    t.add_simple(0xd721, 16, "SDSKIPFIRST")?;
    t.add_simple(0xd722, 16, "SDCUTLAST")?;
    t.add_simple(0xd723, 16, "SDSKIPLAST")?;
    t.add_simple(0xd724, 16, "SDSUBSTR")?;

    t.add_simple(0xd726, 16, "SDBEGINSX")?;
    t.add_simple(0xd727, 16, "SDBEGINSXQ")?;
    t.add_ext(0xd728 >> 3, 13, 8, Box::new(dump_slice_begins_with_const), Box::new(compute_len_slice_begins_with_const))?;
    t.add_simple(0xd730, 16, "SCUTFIRST")?;
    t.add_simple(0xd731, 16, "SSKIPFIRST")?;
    t.add_simple(0xd732, 16, "SCUTLAST")?;
    t.add_simple(0xd733, 16, "SSKIPLAST")?;
    t.add_simple(0xd734, 16, "SUBSLICE")?;

    t.add_simple(0xd736, 16, "SPLIT")?;
    t.add_simple(0xd737, 16, "SPLITQ")?;

    t.add_simple(0xd739, 16, "XCTOS")?;
    t.add_simple(0xd73a, 16, "XLOAD")?;
    t.add_simple(0xd73b, 16, "XLOADQ")?;

    t.add_simple(0xd741, 16, "SCHKBITS")?;
    t.add_simple(0xd742, 16, "SCHKREFS")?;
    t.add_simple(0xd743, 16, "SCHKBITREFS")?;

    t.add_simple(0xd745, 16, "SCHKBITSQ")?;
    t.add_simple(0xd746, 16, "SCHKREFSQ")?;
    t.add_simple(0xd747, 16, "SCHKBITREFSQ")?;
    t.add_simple(0xd748, 16, "PLDREFVAR")?;
    t.add_simple(0xd749, 16, "SBITS")?;
    t.add_simple(0xd74a, 16, "SREFS")?;
    t.add_simple(0xd74b, 16, "SBITREFS")?;
    t.add_fixed(0xd74c >> 2, 14, 2, dump_1c_and(0b11, "PLDREFIDX "))?;
    t.add_fixed(0xd75, 12, 4, Box::new(dump_load_le_int))?;
    t.add_simple(0xd760, 16, "LDZEROES")?;
    t.add_simple(0xd761, 16, "LDONES")?;
    t.add_simple(0xd762, 16, "LDSAME")?;
    t.add_simple(0xd764, 16, "SDEPTH")?;
    t.add_simple(0xd765, 16, "CDEPTH")?;

    Ok(())
}

#[rustfmt::skip]
fn register_continuation_ops(t: &mut OpcodeTable) -> Result<()> {
    // Jump
    t.add_simple(0xd8, 8, "EXECUTE")?;
    t.add_simple(0xd9, 8, "JMPX")?;
    t.add_fixed(0xda, 8, 8, dump_2c("CALLXARGS ", ","))?;
    t.add_fixed(0xdb0, 12, 4, dump_1c_suffix("CALLXARGS ", ",-1"))?;
    t.add_fixed(0xdb1, 12, 4, dump_1c("JMPXARGS "))?;
    t.add_fixed(0xdb2, 12, 4, dump_1c("RETARGS "))?;
    t.add_simple(0xdb30, 16, "RET")?;
    t.add_simple(0xdb31, 16, "RETALT")?;
    t.add_simple(0xdb32, 16, "RETBOOL")?;
    t.add_simple(0xdb34, 16, "CALLCC")?;
    t.add_simple(0xdb35, 16, "JMPXDATA")?;
    t.add_fixed(0xdb36, 16, 8, dump_2c("CALLCCARGS ", ","))?;
    t.add_simple(0xdb38, 16, "CALLXVARARGS")?;
    t.add_simple(0xdb39, 16, "RETVARARGS")?;
    t.add_simple(0xdb3a, 16, "JMPXVARARGS")?;
    t.add_simple(0xdb3b, 16, "CALLCCVARARGS")?;
    t.add_ext(0xdb3c, 16, 0, dump_push_ref("CALLREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xdb3d, 16, 0, dump_push_ref("JMPREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xdb3e, 16, 0, dump_push_ref("JMPREFDATA"), Box::new(compute_len_push_ref))?;
    t.add_simple(0xdb3f, 16, "RETDATA")?;

    // Loops
    t.add_simple(0xdc, 8, "IFRET")?;
    t.add_simple(0xdd, 8, "IFNOTRET")?;
    t.add_simple(0xde, 8, "IF")?;
    t.add_simple(0xdf, 8, "IFNOT")?;
    t.add_simple(0xe0, 8, "IFJMP")?;
    t.add_simple(0xe1, 8, "IFNOTJMP")?;
    t.add_simple(0xe2, 8, "IFELSE")?;
    t.add_ext(0xe300, 16, 0, dump_push_ref("IFREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xe301, 16, 0, dump_push_ref("IFNOTREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xe302, 16, 0, dump_push_ref("IFJMPREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xe303, 16, 0, dump_push_ref("IFNOTJMPREF"), Box::new(compute_len_push_ref))?;
    t.add_simple(0xe304, 16, "CONDSEL")?;
    t.add_simple(0xe305, 16, "CONDSELCHK")?;
    t.add_simple(0xe308, 16, "IFRETALT")?;
    t.add_simple(0xe309, 16, "IFNOTRETALT")?;

    t.add_ext(0xe30d, 16, 0, dump_push_ref("IFREFELSE"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xe30e, 16, 0, dump_push_ref("IFELSEREF"), Box::new(compute_len_push_ref))?;
    t.add_ext(0xe30f, 16, 0, dump_push_ref2("IFREFELSEREF"), Box::new(compute_len_push_ref2))?;
    t.add_fixed(0xe380 >> 6, 10, 6, Box::new(dump_if_bit_jmp))?;
    t.add_ext(0xe3c0 >> 6, 10, 6, Box::new(dump_if_bit_jmpref), Box::new(compute_len_push_ref))?;

    t.add_simple(0xe4, 8, "REPEAT")?;
    t.add_simple(0xe5, 8, "REPEATEND")?;
    t.add_simple(0xe6, 8, "UNTIL")?;
    t.add_simple(0xe7, 8, "UNTILEND")?;
    t.add_simple(0xe8, 8, "WHILE")?;
    t.add_simple(0xe9, 8, "WHILEEND")?;
    t.add_simple(0xea, 8, "AGAIN")?;
    t.add_simple(0xeb, 8, "AGAINEND")?;

    t.add_simple(0xe314, 16, "REPEATBRK")?;
    t.add_simple(0xe315, 16, "REPEATENDBRK")?;
    t.add_simple(0xe316, 16, "UNTILBRK")?;
    t.add_simple(0xe317, 16, "UNTILENDBRK")?;
    t.add_simple(0xe318, 16, "WHILEBRK")?;
    t.add_simple(0xe319, 16, "WHILEENDBRK")?;
    t.add_simple(0xe31a, 16, "AGAINBRK")?;
    t.add_simple(0xe31b, 16, "AGAINENDBRK")?;

    // Cont change
    t.add_fixed(0xec, 8, 8, dump_setcontargs("SETCONTARGS"))?;
    t.add_fixed(0xed0, 12, 4, dump_1c("RETURNARGS "))?;
    t.add_simple(0xed10, 16, "RETURNVARARGS")?;
    t.add_simple(0xed11, 16, "SETCONTVARARGS")?;
    t.add_simple(0xed12, 16, "SETNUMVARARGS")?;
    t.add_simple(0xed1e, 16, "BLESS")?;
    t.add_simple(0xed1f, 16, "BLESSVARARGS")?;

    fn reg_ctr_oprange(t: &mut OpcodeTable, opcode: u32, name: &'static str) -> Result<()> {
        t.add_fixed_range(opcode, opcode + 4, 16, 4, dump_1c(name))?;
        t.add_fixed_range(opcode + 4, opcode + 6, 16, 4, dump_1c(name))?;
        t.add_fixed_range(opcode + 7, opcode + 8, 16, 4, dump_1c(name))
    }

    reg_ctr_oprange(t, 0xed40, "PUSH c")?;
    reg_ctr_oprange(t, 0xed50, "POP c")?;
    reg_ctr_oprange(t, 0xed60, "SETCONTCTR c")?;
    reg_ctr_oprange(t, 0xed70, "SETRETCTR c")?;
    reg_ctr_oprange(t, 0xed80, "SETALTCTR c")?;
    reg_ctr_oprange(t, 0xed90, "POPSAVE c")?;
    reg_ctr_oprange(t, 0xeda0, "SAVECTR c")?;
    reg_ctr_oprange(t, 0xedb0, "SAVEALTCTR c")?;
    reg_ctr_oprange(t, 0xedc0, "PUSSAVEBOTHCTRH c")?;

    t.add_simple(0xede0, 16, "PUSHCTRX")?;
    t.add_simple(0xede1, 16, "POPCTRX")?;
    t.add_simple(0xede2, 16, "SETCONTCTRX")?;

    t.add_simple(0xedf0, 16, "BOOLAND")?;
    t.add_simple(0xedf1, 16, "BOOLOR")?;
    t.add_simple(0xedf2, 16, "COMPOSBOTH")?;
    t.add_simple(0xedf3, 16, "ATEXIT")?;
    t.add_simple(0xedf4, 16, "ATEXITALT")?;
    t.add_simple(0xedf5, 16, "SETEXITALT")?;
    t.add_simple(0xedf6, 16, "THENRET")?;
    t.add_simple(0xedf7, 16, "THENRETALT")?;
    t.add_simple(0xedf8, 16, "INVERT")?;
    t.add_simple(0xedf9, 16, "BOOLEVAL")?;
    t.add_simple(0xedfa, 16, "SAMEALT")?;
    t.add_simple(0xedfb, 16, "SAMEALTSAVE")?;

    t.add_fixed(0xee, 8, 8, dump_setcontargs("BLESSARGS"))?;

    // Dict jump
    t.add_fixed(0xf0, 8, 8, dump_1c_and(0xff, "CALLDICT "))?;
    t.add_fixed(0xf100 >> 6, 10, 14, dump_1c_and(0x3fff, "CALLDICT "))?;
    t.add_fixed(0xf140 >> 6, 10, 14, dump_1c_and(0x3fff, "JMPDICT"))?;
    t.add_fixed(0xf180 >> 6, 10, 14, dump_1c_and(0x3fff, "PREPAREDICT "))?;

    // Exception
    t.add_fixed(0xf200 >> 6, 10, 6, dump_1c_and(0x3f, "THROW "))?;
    t.add_fixed(0xf240 >> 6, 10, 6, dump_1c_and(0x3f, "THROWIF "))?;
    t.add_fixed(0xf280 >> 6, 10, 6, dump_1c_and(0x3f, "THROWIFNOT "))?;
    t.add_fixed(0xf2c0 >> 3, 13, 11, dump_1c_and(0x7ff, "THROW "))?;
    t.add_fixed(0xf2c8 >> 3, 13, 11, dump_1c_and(0x7ff, "THROWARG "))?;
    t.add_fixed(0xf2d0 >> 3, 13, 11, dump_1c_and(0x7ff, "THROWIF "))?;
    t.add_fixed(0xf2d8 >> 3, 13, 11, dump_1c_and(0x7ff, "THROWARGIF "))?;
    t.add_fixed(0xf2e0 >> 3, 13, 11, dump_1c_and(0x7ff, "THROWIFNOT "))?;
    t.add_fixed(0xf2e8 >> 3, 13, 11, dump_1c_and(0x7ff, "THROWARGIFNOT "))?;
    t.add_fixed_range(0xf2f0, 0xf2f6, 16, 3, Box::new(dump_throw_any))?;
    t.add_simple(0xf2ff, 16, "TRY")?;
    t.add_fixed(0xf3, 8, 8, dump_2c("TRYARGS ", ","))?;

    Ok(())
}

#[rustfmt::skip]
fn register_dictionary_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_simple(0xf400, 16, "STDICT")?;
    t.add_simple(0xf401, 16, "SKIPDICT")?;
    t.add_simple(0xf402, 16, "LDDICTS")?;
    t.add_simple(0xf403, 16, "PLDDICTS")?;
    t.add_simple(0xf404, 16, "LDDICT")?;
    t.add_simple(0xf405, 16, "PLDDICT")?;
    t.add_simple(0xf406, 16, "LDDICTQ")?;
    t.add_simple(0xf407, 16, "PLDDICTQ")?;

    t.add_fixed_range(0xf40a, 0xf410, 16, 3, dump_dictop("GET"))?;
    t.add_fixed_range(0xf412, 0xf418, 16, 3, dump_dictop("SET"))?;
    t.add_fixed_range(0xf41a, 0xf420, 16, 3, dump_dictop("SETGET"))?;
    t.add_fixed_range(0xf422, 0xf428, 16, 3, dump_dictop("REPLACE"))?;
    t.add_fixed_range(0xf42a, 0xf430, 16, 3, dump_dictop("REPLACEGET"))?;
    t.add_fixed_range(0xf432, 0xf438, 16, 3, dump_dictop("ADD"))?;
    t.add_fixed_range(0xf43a, 0xf440, 16, 3, dump_dictop("ADDGET"))?;

    t.add_fixed_range(0xf441, 0xf444, 16, 2, dump_dictop2("SETB"))?;
    t.add_fixed_range(0xf445, 0xf448, 16, 2, dump_dictop2("SETGETB"))?;
    t.add_fixed_range(0xf449, 0xf44c, 16, 2, dump_dictop2("REPLACEB"))?;
    t.add_fixed_range(0xf44d, 0xf450, 16, 2, dump_dictop2("REPLACEGETB"))?;
    t.add_fixed_range(0xf451, 0xf454, 16, 2, dump_dictop2("ADDB"))?;
    t.add_fixed_range(0xf455, 0xf458, 16, 2, dump_dictop2("ADDGETB"))?;
    t.add_fixed_range(0xf459, 0xf45c, 16, 2, dump_dictop2("DEL"))?;

    t.add_fixed_range(0xf462, 0xf468, 16, 3, dump_dictop("DELGET"))?;
    t.add_fixed_range(0xf469, 0xf46c, 16, 2, dump_dictop2("GETOPTREF"))?;
    t.add_fixed_range(0xf46d, 0xf470, 16, 2, dump_dictop2("SETGETOPTREF"))?;
    t.add_simple(0xf470, 16, "PFXDICTSET")?;
    t.add_simple(0xf471, 16, "PFXDICTREPLACE")?;
    t.add_simple(0xf472, 16, "PFXDICTADD")?;
    t.add_simple(0xf473, 16, "PFXDICTDEL")?;
    t.add_fixed_range(0xf474, 0xf480, 16, 4, Box::new(dump_dictop_getnear))?;
    t.add_fixed_range(0xf482, 0xf488, 16, 5, dump_dictop("MIN"))?;
    t.add_fixed_range(0xf48a, 0xf490, 16, 5, dump_dictop("MAX"))?;
    t.add_fixed_range(0xf492, 0xf498, 16, 5, dump_dictop("REMMIN"))?;
    t.add_fixed_range(0xf49a, 0xf4a0, 16, 5, dump_dictop("REMMAX"))?;
    t.add_fixed(0xf4a0 >> 2, 14, 2, Box::new(dump_dict_get_exec))?;
    t.add_ext_range(0xf4a400, 0xf4a800, 24, 11, dump_push_const_dict("DICTPUSHCONST"), Box::new(compute_len_push_const_dict))?;
    t.add_simple(0xf4a8, 16, "PFXDICTGETQ")?;
    t.add_simple(0xf4a9, 16, "PFXDICTGET")?;
    t.add_simple(0xf4aa, 16, "PFXDICTGETJMP")?;
    t.add_simple(0xf4ab, 16, "PFXDICTGETEXEC")?;
    t.add_ext_range(0xf4ac00, 0xf4b000, 24, 11, dump_push_const_dict("PFXDICTSWITCH"), Box::new(compute_len_push_const_dict))?;
    t.add_fixed_range(0xf4b1, 0xf4b4, 16, 3, dump_subdictop2("GET"))?;
    t.add_fixed_range(0xf4b5, 0xf4b8, 16, 3, dump_subdictop2("RPGET"))?;
    t.add_fixed(0xf4bc >> 2, 14, 2, Box::new(dump_dict_get_exec))?;

    Ok(())
}

fn register_ton_ops(t: &mut OpcodeTable) -> Result<()> {
    // Basic gas
    t.add_simple(0xf800, 16, "ACCEPT")?;
    t.add_simple(0xf801, 16, "SETGASLIMIT")?;
    t.add_simple(0xf802, 16, "BUYGAS")?;
    t.add_simple(0xf804, 16, "GRAMTOGAS")?;
    t.add_simple(0xf805, 16, "GASTOGRAM")?;
    t.add_simple(0xf806, 16, "GASREMAINING")?;
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
    t.add_simple(0xf82a, 16, "MYCODE")?;
    t.add_simple(0xf82b, 16, "INITCODEHASH")?;
    t.add_simple(0xf82c, 16, "STORAGEFEE")?;
    t.add_simple(0xf82d, 16, "SEQNO")?;
    t.add_fixed_range(0xf82e, 0xf830, 16, 4, dump_1c("GETPARAM "))?;
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

#[rustfmt::skip]
fn register_debug_ops(t: &mut OpcodeTable) -> Result<()> {
    t.add_fixed_range(0xfe00, 0xfef0, 16, 8, dump_1c_and(0xff, "DEBUG "))?;
    t.add_ext(0xfef, 12, 4, Box::new(dump_dummy_debug_str), Box::new(compute_len_debug_str))?;

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
        let bits = std::cmp::min(MAX_OPCODE_BITS, slice.size_bits());
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
            slice.skip_first(self.bits, 0)?;
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
            slice.skip_first(self.total_bits, 0)?;
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
            Some((self.instr_len)(
                slice,
                opcode >> (MAX_OPCODE_BITS - self.total_bits),
                self.total_bits,
            ))
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

fn dump_1c_suffix(prefix: &'static str, suffix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(f, "{prefix}{}{suffix}", args & 0xf)?;
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

fn dump_2sr_adj(adj: i32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}s{},s{}",
            ((args as i32 >> 4) & 0xf) - ((adj >> 4) & 0xf),
            (args as i32 & 0xf) - (adj & 0xf)
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

fn dump_3sr_adj(adj: i32, prefix: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        write!(
            f,
            "{prefix}s{},s{},s{}",
            ((args as i32 >> 8) & 0xf) - ((adj >> 8) & 0xf),
            ((args as i32 >> 4) & 0xf) - ((adj >> 4) & 0xf),
            (args as i32 & 0xf) - (adj & 0xf)
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
    let x = ((args as i32 + 5) & 0xf) - 5;
    write!(f, "PUSHINT {x}")?;
    Ok(())
}

fn dump_op_tinyint8(name: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        let x = args as i8;
        write!(f, "{name}{x}")?;
        Ok(())
    })
}

fn dump_push_smallint(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let x = args as i16;
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

    cs.skip_first(bits, 0)?;

    let mut bytes = [0u8; 33];
    let rem = value_len % 8;
    let mut int = num_bigint::BigUint::from_bytes_be(cs.load_raw(&mut bytes, value_len)?);
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
        cs.skip_first(bits, 0)?;
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

fn dump_push_ref2(name: &'static str) -> Box<FnDumpInstr> {
    Box::new(move |cs, _, bits, f| {
        if !cs.has_remaining(0, 2) {
            return Ok(());
        }
        cs.skip_first(bits, 0)?;
        let cell1 = cs.load_reference()?;
        let cell2 = cs.load_reference()?;
        write!(f, "{name} ({}) ({})", cell1.repr_hash(), cell2.repr_hash())?;
        Ok(())
    })
}

fn compute_len_push_ref2(cs: &CellSlice<'_>, _: u32, bits: u16) -> (u16, u8) {
    if cs.has_remaining(0, 2) {
        (bits, 2)
    } else {
        (0, 0)
    }
}

fn dump_if_bit_jmp(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let neg = if args & 0x20 != 0 { "N" } else { "" };
    write!(f, "IF{neg}BITJMP {}", args & 0x1f)?;
    Ok(())
}

fn dump_if_bit_jmpref(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    if cs.is_refs_empty() {
        return Ok(());
    }
    cs.skip_first(bits, 1)?;
    let neg = if args & 0x20 != 0 { "N" } else { "" };
    write!(f, "IF{neg}BITJMPREF {}", args & 0x1f)?;
    Ok(())
}

fn dump_setcontargs(name: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        let copy = (args >> 4) & 0xf;
        let more = ((args as i32 + 1) & 0xf) - 1;
        write!(f, "{name} {copy},{more}")?;
        Ok(())
    })
}

fn dump_throw_any(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let param = if args & 0b001 != 0 { "ARG" } else { "" };
    let cond = if args & 0b110 != 0 {
        "IF"
    } else if args & 0b100 != 0 {
        "IFNOT"
    } else {
        ""
    };
    write!(f, "THROW{param}{cond}")?;
    Ok(())
}

fn dump_dictop(name: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        f.write_str("DICT")?;
        if args & 0b100 != 0 {
            f.write_str(if args & 0b010 != 0 { "U" } else { "I" })?;
        }
        f.write_str(name)?;
        if args & 0b001 != 0 {
            f.write_str("REF")?;
        }
        Ok(())
    })
}

fn dump_dictop2(name: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        f.write_str("DICT")?;
        if args & 0b10 != 0 {
            f.write_str(if args & 0b01 != 0 { "U" } else { "I" })?;
        }
        f.write_str(name)?;
        Ok(())
    })
}

fn dump_subdictop2(name: &'static str) -> Box<FnDumpArgInstr> {
    Box::new(move |_, args, f| {
        f.write_str("SUBDICT")?;
        if args & 0b10 != 0 {
            f.write_str(if args & 0b01 != 0 { "U" } else { "I" })?;
        }
        f.write_str(name)?;
        Ok(())
    })
}

fn dump_dictop_getnear(
    _: &mut CellSlice<'_>,
    args: u32,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let num = if args & 0b1100 != 0 {
        "U"
    } else if args & 0b1000 != 0 {
        "I"
    } else {
        ""
    };
    let dir = if args & 0b0010 != 0 { "PREV" } else { "NEXT" };
    let exact = if args & 0b0001 != 0 { "EQ" } else { "" };
    write!(f, "DICT{num}GET{dir}{exact}")?;
    Ok(())
}

fn dump_dict_get_exec(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let sign = if args & 0b001 != 0 { "U" } else { "I" };
    let flow = if args & 0b010 != 0 { "EXEC" } else { "JMP" };
    let idx = if args & 0b100 != 0 { "Z" } else { "" };
    write!(f, "DICT{sign}GET{flow}{idx}")?;
    Ok(())
}

fn dump_push_const_dict(name: &'static str) -> Box<FnDumpInstr> {
    Box::new(move |cs, _, bits, f| {
        if !cs.has_remaining(bits, 1) {
            return Ok(());
        }
        cs.skip_first(bits - 11, 0)?;
        let slice = cs.get_prefix(1, 1);
        cs.skip_first(1, 1)?;
        let n = cs.load_uint(10)?;

        write!(f, "{name} {n} (x{})", slice.display_data())?;
        Ok(())
    })
}

fn compute_len_push_const_dict(cs: &CellSlice<'_>, _: u32, bits: u16) -> (u16, u8) {
    if cs.has_remaining(bits, 1) {
        (bits, 1)
    } else {
        (0, 0)
    }
}

fn dump_push_slice(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = (args as u16 & 0xf) * 8 + 4;
    dump_push_slice_ext(cs, (slice_bits, 0), bits, "PUSHSLICE", f)
}

fn compute_len_push_slice(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = (args as u16 & 0xf) * 8 + 4;
    compute_len_push_slice_ext(cs, (slice_bits, 0), bits)
}

fn dump_push_slice_r(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = (args as u16 & 31) * 8 + 1;
    let slice_refs = ((args as u8 >> 5) & 0b11) + 1;
    dump_push_slice_ext(cs, (slice_bits, slice_refs), bits, "PUSHSLICE", f)
}

fn compute_len_push_slice_r(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = (args as u16 & 31) * 8 + 1;
    let slice_refs = ((args as u8 >> 5) & 0b11) + 1;
    compute_len_push_slice_ext(cs, (slice_bits, slice_refs), bits)
}

fn dump_push_slice_r2(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = (args as u16 & 127) * 8 + 6;
    let slice_refs = ((args >> 7) & 7) as u8;
    dump_push_slice_ext(cs, (slice_bits, slice_refs), bits, "PUSHSLICE", f)
}

fn compute_len_push_slice_r2(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = (args as u16 & 127) * 8 + 6;
    let slice_refs = ((args >> 7) & 7) as u8;
    compute_len_push_slice_ext(cs, (slice_bits, slice_refs), bits)
}

fn dump_push_cont(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = (args as u16 & 127) * 8;
    let slice_refs = ((args >> 7) & 0b11) as u8;
    dump_push_slice_ext(cs, (slice_bits, slice_refs), bits, "PUSHCONT", f)
}

fn compute_len_push_cont(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = (args as u16 & 127) * 8;
    let slice_refs = ((args >> 7) & 0b11) as u8;
    compute_len_push_slice_ext(cs, (slice_bits, slice_refs), bits)
}

fn dump_push_cont_simple(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = (args as u16 & 0xf) * 8;
    dump_push_slice_ext(cs, (slice_bits, 0), bits, "PUSHCONT", f)
}

fn compute_len_push_cont_simple(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = (args as u16 & 0xf) * 8;
    compute_len_push_slice_ext(cs, (slice_bits, 0), bits)
}

fn dump_store_const_slice(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = (args as u16 & 7) * 8 + 2;
    let slice_refs = (args as u8 >> 3) & 0b11;
    dump_push_slice_ext(cs, (slice_bits, slice_refs), bits, "STSLICECONST", f)
}

fn compute_len_store_const_slice(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = (args as u16 & 7) * 8 + 2;
    let slice_refs = (args as u8 >> 3) & 0b11;
    compute_len_push_slice_ext(cs, (slice_bits, slice_refs), bits)
}

fn dump_push_slice_ext(
    cs: &mut CellSlice<'_>,
    slice_len: (u16, u8),
    bits: u16,
    name: &'static str,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let (slice_bits, slice_refs) = slice_len;
    if !cs.has_remaining(bits + slice_bits, slice_refs) {
        return Ok(());
    }
    cs.skip_first(bits, 0)?;
    let mut slice = cs.get_prefix(slice_bits, slice_refs);
    cs.skip_first(slice_bits, slice_refs)?;
    slice_trim_end(&mut slice)?;

    write!(f, "{name} x{}", slice.display_data())?;
    if !slice.is_refs_empty() {
        write!(f, ",{}", slice.size_refs())?;
    }
    Ok(())
}

fn compute_len_push_slice_ext(cs: &CellSlice<'_>, slice_len: (u16, u8), bits: u16) -> (u16, u8) {
    let (slice_bits, slice_refs) = slice_len;
    let bits = bits + slice_bits;
    if cs.has_remaining(bits, slice_refs) {
        (bits, slice_refs)
    } else {
        (0, 0)
    }
}

fn dump_dummy_debug_str(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let slice_bits = ((args as u16 & 0xf) + 1) * 8;
    if !cs.has_remaining(bits + slice_bits, 0) {
        return Ok(());
    }
    cs.skip_first(bits, 0)?;
    let mut slice = cs.get_prefix(slice_bits, 0);
    cs.skip_first(slice_bits, 0)?;
    slice_trim_end(&mut slice)?;

    write!(f, "DEBUGSTR x{}", slice.display_data())?;
    if !slice.is_refs_empty() {
        write!(f, ",{}", slice.size_refs())?;
    }
    Ok(())
}

fn compute_len_debug_str(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let bits = bits + ((args as u16 & 0xf) + 1) * 8;
    (bits * cs.has_remaining(bits, 0) as u16, 0)
}

fn dump_store_int_var(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let signed = if args & 0b001 != 0 { "I" } else { "U" };
    write!(f, "ST{signed}X")?;
    if args & 0b010 != 0 {
        f.write_str("R")?;
    }
    if args & 0b100 != 0 {
        f.write_str("Q")?;
    }
    Ok(())
}

fn dump_store_int_fixed(
    _: &mut CellSlice<'_>,
    args: u32,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let bits = (args & 0xff) + 1;
    let signed = if args & 0x100 != 0 { "I" } else { "U" };
    write!(f, "ST{signed}")?;
    if args & 0x200 != 0 {
        f.write_str("R")?;
    }
    if args & 0x400 != 0 {
        f.write_str("Q")?;
    }
    write!(f, " {bits}")?;
    Ok(())
}

fn dump_store_cont_ref(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let refs = (args as u8 & 1) + 1;
    if !cs.has_remaining(0, refs) {
        return Ok(());
    }
    cs.skip_first(bits, refs)?;
    if refs > 1 {
        write!(f, "STREF{refs}CONST")?;
    } else {
        f.write_str("STREFCONST")?;
    }
    Ok(())
}

fn compute_len_store_const_ref(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let refs = (args as u8 & 1) + 1;
    if cs.has_remaining(0, refs) {
        (bits, refs)
    } else {
        (0, 0)
    }
}

fn dump_store_le_int(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let signed = if args & 0b01 != 0 { "I" } else { "U" };
    let long = if args & 0b10 != 0 { "8" } else { "4" };
    write!(f, "ST{signed}LE{long}")?;
    Ok(())
}

fn dump_load_int_var(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let ld = if args & 0b010 != 0 { "PLD" } else { "LD" };
    let signed = if args & 0b001 != 0 { "UX" } else { "IX" };
    let quiet = if args & 0b100 != 0 { "Q" } else { "" };
    write!(f, "{ld}{signed}{quiet}")?;
    Ok(())
}

fn dump_load_int_fixed2(
    _: &mut CellSlice<'_>,
    args: u32,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let ld = if args & 0x200 != 0 { "PLD" } else { "LD" };
    let signed = if args & 0x100 != 0 { "U" } else { "I" };
    let quiet = if args & 0x400 != 0 { "Q" } else { "" };
    write!(f, "{ld}{signed}{quiet} {}", (args & 0xff) + 1)?;
    Ok(())
}

fn dump_preload_uint_fixed_0e(
    _: &mut CellSlice<'_>,
    args: u32,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    write!(f, "PLDUZ {}", ((args & 7) + 1) << 5)?;
    Ok(())
}

fn dump_load_slice(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let p = if args & 0b01 != 0 { "P" } else { "" };
    let q = if args & 0b10 != 0 { "Q" } else { "" };
    write!(f, "{p}LDSLICEX{q}")?;
    Ok(())
}

fn dump_load_slice_fixed2(
    _: &mut CellSlice<'_>,
    args: u32,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let ld = if args & 0x100 != 0 {
        "PLDSLICE"
    } else {
        "LDSLICE"
    };
    let q = if args & 0x200 != 0 { "Q" } else { "" };
    write!(f, "{ld}{q} {}", (args & 0xff) + 1)?;
    Ok(())
}

fn dump_slice_begins_with_const(
    cs: &mut CellSlice<'_>,
    args: u32,
    bits: u16,
    f: &mut dyn std::fmt::Write,
) -> Result<()> {
    let name = if args & 128 != 0 {
        "SDBEGINSQ"
    } else {
        "SDBEGINS"
    };
    let slice_bits = ((args & 127) * 8 + 3) as u16;
    dump_push_slice_ext(cs, (slice_bits, 0), bits, name, f)
}

fn compute_len_slice_begins_with_const(cs: &CellSlice<'_>, args: u32, bits: u16) -> (u16, u8) {
    let slice_bits = ((args & 127) * 8 + 3) as u16;
    compute_len_push_slice_ext(cs, (slice_bits, 0), bits)
}

fn dump_load_le_int(_: &mut CellSlice<'_>, args: u32, f: &mut dyn std::fmt::Write) -> Result<()> {
    let ld = if args & 0b0100 != 0 { "PLD" } else { "LD" };
    let signed = if args & 0b0001 != 0 { "I" } else { "U" };
    let size = if args & 0b0010 != 0 { "8" } else { "4" };
    let quiet = if args & 0b1000 != 0 { "Q" } else { "" };
    write!(f, "{ld}{signed}LE{size}{quiet}")?;
    Ok(())
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
        let mut y = 0;
        if has_y {
            y = (args & 0xff) + 1;
            args >>= 8;
        }
        let round_mode = args & 0b11;
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
        let mut y = 0;
        if has_y {
            y = (args & 0xff) + 1;
            args >>= 8;
        }
        let round_mode = args & 0b11;
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
        let mut y = 0;
        if has_y {
            y = (args & 0xff) + 1;
            args >>= 8;
        }
        let round_mode = args & 0b11;
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

fn slice_trim_end(slice: &mut CellSlice<'_>) -> Result<()> {
    let bits = slice.size_bits();
    if bits == 0 {
        return Ok(());
    }
    let mut trailing = slice_trailing_zeros(slice)?;
    if bits != trailing {
        trailing = trailing.saturating_add(1);
    }
    *slice = slice.get_prefix(bits - trailing, slice.size_refs());
    Ok(())
}

fn slice_trailing_zeros(slice: &CellSlice<'_>) -> Result<u16> {
    let mut bits = slice.size_bits();
    if bits == 0 {
        return Ok(0);
    }

    let offs = bits % 8;
    let mut res = offs;
    if offs > 0 {
        let last = slice.get_small_uint(bits & !0b111, offs)?;
        let c = last.trailing_zeros() as u16;
        if c < offs || res >= bits {
            return Ok(std::cmp::min(c, bits));
        }
    }

    bits -= offs;
    while bits >= 32 {
        bits -= 32;
        let v = slice.get_u32(bits)?;
        if v != 0 {
            return Ok(res + v.trailing_zeros() as u16);
        }
        res += 32;
    }

    while bits >= 8 {
        bits -= 8;
        let v = slice.get_u8(bits)?;
        if v != 0 {
            return Ok(res + v.trailing_zeros() as u16);
        }
        res += 8;
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_opcodes_are_valid() {
        assert!(!cp0().opcodes.is_empty());
    }
}
